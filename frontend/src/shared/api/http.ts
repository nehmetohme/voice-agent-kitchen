/**
 * The fetch mutator behind the orval-generated client (see orval.config.ts). Every API call
 * goes through `http`, which:
 * - asks for JSON and sends the session cookie (same origin),
 * - turns non-2xx responses into an `ApiError` carrying RFC 9457 problem details,
 * - parses the response with the Zod schema orval passes in, so a backend that drifts from the
 *   contract fails loudly here instead of deep inside a view (Postel: strict output).
 */
import { z, type ZodType } from 'zod';

const problemSchema = z.looseObject({
  type: z.string().optional(),
  title: z.string().optional(),
  status: z.number().int().optional(),
  detail: z.string().optional(),
  instance: z.string().optional(),
});

/** RFC 9457 problem details, as the backend returns them (`application/problem+json`). */
export type Problem = z.infer<typeof problemSchema>;

/** The server answered with a non-2xx status. */
export class ApiError extends Error {
  override readonly name = 'ApiError';
  readonly status: number;
  readonly problem: Problem;

  constructor(status: number, problem: Problem) {
    super(problem.detail ?? problem.title ?? `Request failed with status ${status}`);
    this.status = status;
    this.problem = problem;
  }
}

/** The server answered 2xx, but the body does not match the contract. */
export class ContractViolationError extends Error {
  override readonly name = 'ContractViolationError';
  readonly url: string;
  readonly issues: z.core.$ZodIssue[];

  constructor(url: string, issues: z.core.$ZodIssue[]) {
    super(`Response from ${url} does not match the API contract`);
    this.url = url;
    this.issues = issues;
  }
}

export interface HttpOptions<T> extends RequestInit {
  /**
   * Response schema; orval passes it when `includeZodSchemaInArguments` is on. Generated calls
   * type their options as `Parameters<typeof http>[1]` (that is, `HttpOptions<unknown>`), so a
   * call without a response body (`204`, `Promise<void>`) must accept a `ZodType<unknown>` too.
   */
  schema?: ZodType<T> | ZodType;
}

async function readBody(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text === '') return undefined;
  const isJson = /[/+]json\b/.test(response.headers.get('content-type') ?? '');
  if (!isJson) return text;
  const body: unknown = JSON.parse(text);
  return body;
}

function toProblem(status: number, body: unknown): Problem {
  const parsed = problemSchema.safeParse(body);
  if (parsed.success) return { ...parsed.data, status };
  return typeof body === 'string' && body !== '' ? { status, detail: body } : { status };
}

export async function http<T>(url: string, options: HttpOptions<T> = {}): Promise<T> {
  const { schema, ...init } = options;
  const headers = new Headers(init.headers);
  if (!headers.has('accept')) headers.set('accept', 'application/json, application/problem+json');

  const response = await fetch(url, { ...init, headers, credentials: 'same-origin' });
  const body = await readBody(response);
  if (!response.ok) throw new ApiError(response.status, toProblem(response.status, body));
  if (schema === undefined) {
    // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- orval omits the schema only for void or primitive responses
    return body as T;
  }
  const parsed = schema.safeParse(body);
  if (!parsed.success) throw new ContractViolationError(url, parsed.error.issues);
  // oxlint-disable-next-line typescript/no-unsafe-type-assertion -- orval pairs every call's T with its own schema; ZodType<unknown> appears only in the shared options type
  return parsed.data as T;
}
