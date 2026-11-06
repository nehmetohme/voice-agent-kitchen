/**
 * Deterministic stand-ins for the two models a test run talks to, so end-to-end test runs need
 * no GPU and always end the same way. `backend-server.ts` starts this on a free port and points
 * the real `vak-server` at it; the server's own adapters speak to it as they would to the local
 * models. Part of contract test-runs v1 (its e2e spec relies on these answers): never edit it
 * once it is tagged `contract/test-runs/v1`.
 *
 * The LLM (`POST /v1/chat/completions`, an OpenAI-compatible event stream):
 * - A request that offers the `end_call` tool comes from the simulated caller. By its own
 *   lines so far, it says "Hi. Yes, I have a minute.", then "Please stop calling me." when its
 *   instructions mention stopping and "About an eight, thanks." otherwise, then "Thanks, bye."
 *   with the `end_call` tool.
 * - Any other request is the agent. Its first reply asks the survey question ("Thanks for
 *   taking our survey. How was your visit, from one to ten?"), every later one says "Got it,
 *   thank you."
 *
 * The decision model (`POST /v1/systemone`): `opt_out` is 0.97 when the last caller turn
 * asks to stop, else 0.02; every other yes/no question (a rubric's) is 0.9.
 *
 * Each answer waits `FAKE_MODELS_DELAY_MS` (default 150 ms), so a run is visibly in progress.
 */
import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';

export const FAKE_LLM_MODEL = 'fake-llm';
export const FAKE_DECISION_MODEL = 'fake-decisions';

const CALLER_TOOL = 'end_call';
const DELAY_MS = Number(process.env['FAKE_MODELS_DELAY_MS'] ?? '150');

interface Message {
  role: string;
  content: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function messagesOf(body: Record<string, unknown>): Message[] {
  const raw = Array.isArray(body['messages']) ? body['messages'] : [];
  return raw.filter(isRecord).map((message) => ({
    role: typeof message['role'] === 'string' ? message['role'] : '',
    content: typeof message['content'] === 'string' ? message['content'] : '',
  }));
}

function offersCallerTool(body: Record<string, unknown>): boolean {
  const tools = Array.isArray(body['tools']) ? body['tools'] : [];
  return tools.some(
    (tool) =>
      isRecord(tool) && isRecord(tool['function']) && tool['function']['name'] === CALLER_TOOL,
  );
}

interface Reply {
  text: string;
  hangUp: boolean;
}

function callerReply(messages: Message[]): Reply {
  const turn = messages.filter((message) => message.role === 'assistant').length;
  const instructions = messages[0]?.content.toLowerCase() ?? '';
  if (turn === 0) return { text: 'Hi. Yes, I have a minute.', hangUp: false };
  if (turn === 1) {
    const text = instructions.includes('stop')
      ? 'Please stop calling me.'
      : 'About an eight, thanks.';
    return { text, hangUp: false };
  }
  return { text: 'Thanks, bye.', hangUp: true };
}

function agentReply(messages: Message[]): Reply {
  const answered = messages.filter((message) => message.role === 'assistant').length;
  // The disclosure is the agent's first line; the survey question comes with the first reply.
  const text =
    answered <= 1
      ? 'Thanks for taking our survey. How was your visit, from one to ten?'
      : 'Got it, thank you.';
  return { text, hangUp: false };
}

function chunk(delta: Record<string, unknown>, finish: string | null = null): string {
  const event = { choices: [{ index: 0, delta, finish_reason: finish }], model: FAKE_LLM_MODEL };
  return `data: ${JSON.stringify(event)}\n\n`;
}

function streamReply(response: ServerResponse, reply: Reply): void {
  response.writeHead(200, { 'content-type': 'text/event-stream' });
  response.write(chunk({ content: reply.text }));
  if (reply.hangUp) {
    const call = {
      index: 0,
      id: 'call_1',
      type: 'function',
      function: { name: CALLER_TOOL, arguments: '{}' },
    };
    response.write(chunk({ tool_calls: [call] }));
    response.write(chunk({}, 'tool_calls'));
  } else {
    response.write(chunk({}, 'stop'));
  }
  response.end('data: [DONE]\n\n');
}

function lastCallerTurn(body: Record<string, unknown>): string {
  const state = isRecord(body['state']) ? body['state'] : {};
  const turns = Array.isArray(state['turns']) ? state['turns'].filter(isRecord) : [];
  const caller = turns.findLast((turn) => turn['speaker'] === 'caller');
  return typeof caller?.['text'] === 'string' ? caller['text'].toLowerCase() : '';
}

function decide(body: Record<string, unknown>): unknown {
  const questions = isRecord(body['questions']) ? body['questions'] : {};
  const stop = lastCallerTurn(body).includes('stop');
  const answers = Object.fromEntries(
    Object.keys(questions).map((id) => {
      const noul = id === 'opt_out' ? (stop ? 0.97 : 0.02) : 0.9;
      return [id, { type: 'noul', noul }];
    }),
  );
  return { model: FAKE_DECISION_MODEL, answers };
}

async function readJson(request: IncomingMessage): Promise<Record<string, unknown>> {
  const parts: Buffer[] = [];
  for await (const part of request) {
    if (part instanceof Uint8Array) parts.push(Buffer.from(part));
  }
  const parsed: unknown = JSON.parse(Buffer.concat(parts).toString('utf8') || '{}');
  return isRecord(parsed) ? parsed : {};
}

const pause = () => new Promise((resolve) => setTimeout(resolve, DELAY_MS));

async function handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
  const body = await readJson(request);
  await pause();
  if (request.method === 'POST' && request.url === '/v1/chat/completions') {
    const messages = messagesOf(body);
    streamReply(response, offersCallerTool(body) ? callerReply(messages) : agentReply(messages));
    return;
  }
  if (request.method === 'POST' && request.url === '/v1/systemone') {
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(JSON.stringify(decide(body)));
    return;
  }
  response.writeHead(404).end();
}

/** Starts the fake models on a free local port; resolves with their base URL. */
export async function startFakeModels(): Promise<{ baseUrl: string; server: Server }> {
  const server = createServer((request, response) => {
    handle(request, response).catch(() => {
      if (!response.headersSent) response.writeHead(400);
      response.end();
    });
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('the fake models did not get a TCP port');
  }
  return { baseUrl: `http://127.0.0.1:${address.port}`, server };
}
