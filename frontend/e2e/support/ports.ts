/**
 * Ports for one worktree's end-to-end run. Parallel worktrees each start their own web server,
 * API server and database, so each needs its own ports. Unless an environment variable sets a
 * port, it is derived from a hash of the worktree path: the same worktree always gets the same
 * ports, and two worktrees rarely share a slot.
 *
 * | Port | Variable      | Default range |
 * | ---- | ------------- | ------------- |
 * | web  | `E2E_PORT`    | 4200–4999     |
 * | api  | `E2E_API_PORT`| 14200–14999   |
 * | db   | `E2E_DB_PORT` | 24200–24999   |
 *
 * The ranges sit below the OS ephemeral range (49152+) and away from common dev ports.
 */
import { createHash } from 'node:crypto';

export interface E2ePorts {
  /** `vite preview`, the base URL of every test. */
  web: number;
  /** `vak-server`, behind the preview server's `/api` proxy. */
  api: number;
  /** The throwaway Postgres container. */
  db: number;
}

const SLOTS = 800;
const BASE = { web: 4200, api: 14_200, db: 24_200 } as const;
const VARIABLES = { web: 'E2E_PORT', api: 'E2E_API_PORT', db: 'E2E_DB_PORT' } as const;

/** A stable slot in [0, SLOTS) for `worktree`. */
export function slotFor(worktree: string): number {
  const digest = createHash('sha256').update(worktree).digest();
  return digest.readUInt32BE(0) % SLOTS;
}

function portFrom(variable: string, raw: string | undefined): number | undefined {
  if (raw === undefined || raw.trim() === '') return undefined;
  const port = Number(raw);
  if (!Number.isInteger(port) || port < 1024 || port > 65_535) {
    throw new Error(`${variable} must be a whole number from 1024 to 65535, not "${raw}"`);
  }
  return port;
}

/** The ports for `worktree`, each overridable through its environment variable. */
export function e2ePorts(
  worktree: string,
  env: Readonly<Record<string, string | undefined>> = process.env,
): E2ePorts {
  const slot = slotFor(worktree);
  const port = (name: keyof E2ePorts) =>
    portFrom(VARIABLES[name], env[VARIABLES[name]]) ?? BASE[name] + slot;
  return { web: port('web'), api: port('api'), db: port('db') };
}
