/**
 * Pure rules of the purpose editor: what changed since the last save, and edits to tools and
 * guardrails. `use-purpose-draft.ts` and the views run them.
 */
import type {
  BuiltinTool,
  Escalation,
  HttpToolDraft,
  PurposeSettings,
  SectionKey,
  ToolInput,
  ToolSettings,
} from '../api/purpose-models';

const SECTIONS = [
  'overview',
  'tools',
  'guardrails',
  'compliance',
] as const satisfies readonly SectionKey[];

/** The sections of `draft` that differ from `saved`: what a save sends. */
export function changedSections(
  saved: PurposeSettings,
  draft: PurposeSettings,
): Partial<PurposeSettings> {
  const changes: Partial<PurposeSettings> = {};
  for (const section of SECTIONS) {
    if (JSON.stringify(saved[section]) !== JSON.stringify(draft[section])) {
      Object.assign(changes, { [section]: draft[section] });
    }
  }
  return changes;
}

/** Turns a built-in tool on or off; a locked one stays on. */
export function toggleBuiltin(
  tools: ToolSettings,
  tool: BuiltinTool,
  on: boolean,
  locked: boolean,
): ToolSettings {
  if (locked) return tools;
  const without = tools.builtins.filter((name) => name !== tool);
  return { ...tools, builtins: on ? [...without, tool] : without };
}

/** A new HTTP tool's starting values: POST, the default timeout, confirm first. */
export function newHttpTool(timeoutMs: number): HttpToolDraft {
  return {
    name: '',
    description: '',
    method: 'POST',
    url: 'https://',
    timeoutMs,
    secret: '',
    confirm: true,
    inputs: [],
  };
}

export function newToolInput(): ToolInput {
  return { name: '', type: 'string', description: '', required: true };
}

/** `tools` with `tool` in place of the one named `replacing`, or added at the end. */
export function putHttpTool(
  tools: ToolSettings,
  tool: HttpToolDraft,
  replacing: string | null,
): ToolSettings {
  const at = replacing === null ? -1 : tools.http.findIndex((http) => http.name === replacing);
  const http = at === -1 ? [...tools.http, tool] : tools.http.with(at, tool);
  return { ...tools, http };
}

export function removeHttpTool(tools: ToolSettings, name: string): ToolSettings {
  return { ...tools, http: tools.http.filter((tool) => tool.name !== name) };
}

/** LLM-safe: lowercase letters, digits and `_`, starting with a letter. */
const IDENTIFIER = /^[a-z][a-z0-9_]{0,63}$/;

/** What to fix in a tool before it is added, by field; empty when it can be added. */
export function checkHttpTool(
  tool: HttpToolDraft,
  others: readonly string[],
  builtins: readonly string[],
): Partial<Record<'name' | 'description' | 'url' | 'inputs', string>> {
  const errors: Partial<Record<'name' | 'description' | 'url' | 'inputs', string>> = {};
  const name = tool.name.trim();
  if (!IDENTIFIER.test(name)) {
    errors.name = 'Use lowercase letters, digits and _, starting with a letter.';
  } else if (builtins.includes(name) || others.includes(name)) {
    errors.name = 'Another tool has this name.';
  }
  if (tool.description.trim() === '') errors.description = 'Say what the tool does.';
  if (!/^https:\/\/[^/\s]+\.[^/\s]+/i.test(tool.url.trim())) {
    errors.url = 'Use an https:// address with a host name.';
  }
  const names = tool.inputs.map((input) => input.name.trim());
  if (names.some((input) => !IDENTIFIER.test(input))) {
    errors.inputs = 'Name each input with lowercase letters, digits and _.';
  } else if (new Set(names).size !== names.length) {
    errors.inputs = 'Give each input its own name.';
  }
  return errors;
}

/** A new escalation rule's starting values. */
export function newEscalation(): Escalation {
  return { when: '', action: 'transfer', message: '' };
}

/** The host of an https URL, lower case, or `''`. */
export function hostOf(url: string): string {
  const match = /^https:\/\/([^/?#:]+)/i.exec(url.trim());
  return match?.[1]?.toLowerCase() ?? '';
}

/** Whether the allow-list (exact hosts and `*.` wildcards) lets a tool call `host`. */
export function hostAllowed(host: string, allowed: readonly string[]): boolean {
  return allowed.some((entry) =>
    entry.startsWith('*.')
      ? host.endsWith(entry.slice(1)) && host !== entry.slice(2)
      : entry === host,
  );
}
