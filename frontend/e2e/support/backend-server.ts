/**
 * The real backend for end-to-end tests: a throwaway Postgres in Docker, then `vak-server
 * migrate` and `vak-server serve` against it. Playwright starts this as a webServer
 * (playwright.config.ts) and stops it with SIGTERM; the container goes with it.
 *
 *   node e2e/support/backend-server.ts
 *
 * Each worktree gets its own ports and container (support/ports.ts), so parallel runs never
 * share a database. The server runs in the local profile with a fixed dev operator, because
 * there is no Databricks proxy to forward an identity. Its LLM and decision model are the
 * deterministic fakes of `fake-models.ts`, on a free local port, so test runs work and always
 * end the same way; it has no speech models, so it refuses voice calls.
 */
import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { resolve } from 'node:path';

import { FAKE_DECISION_MODEL, FAKE_LLM_MODEL, startFakeModels } from './fake-models.ts';
import { e2ePorts, slotFor } from './ports.ts';

const IMAGE = process.env['E2E_DB_IMAGE'] ?? 'postgres:18.6-alpine';
const DEV_OPERATOR = 'e2e@example.com';

const worktree = resolve(import.meta.dirname, '..', '..', '..');
const backend = resolve(worktree, 'backend');
const ports = e2ePorts(worktree);
const container = `vak-e2e-db-${slotFor(worktree)}`;
const databaseUrl = `postgres://postgres@127.0.0.1:${ports.db}/postgres`;

function run(command: string, args: string[], env: NodeJS.ProcessEnv = {}): void {
  execFileSync(command, args, { cwd: backend, stdio: 'inherit', env: { ...process.env, ...env } });
}

function removeContainer(): void {
  try {
    execFileSync('docker', ['rm', '--force', container], { stdio: 'ignore' });
  } catch {
    // Already gone.
  }
}

function startDatabase(): void {
  removeContainer();
  run('docker', [
    'run',
    '--detach',
    '--quiet',
    '--rm',
    '--name',
    container,
    '--publish',
    `127.0.0.1:${ports.db}:5432`,
    '--env',
    'POSTGRES_HOST_AUTH_METHOD=trust',
    '--tmpfs',
    '/var/lib/postgresql',
    IMAGE,
    '-c',
    'fsync=off',
    '-c',
    'synchronous_commit=off',
  ]);
  for (let attempt = 0; attempt < 120; attempt += 1) {
    try {
      execFileSync('docker', ['exec', container, 'pg_isready', '--quiet', '--host', '127.0.0.1'], {
        stdio: 'ignore',
      });
      return;
    } catch {
      execFileSync('sleep', ['0.5']);
    }
  }
  throw new Error(`Postgres in ${container} did not become ready within 60 s`);
}

let server: ChildProcess | undefined;
const models = await startFakeModels();

function stop(): void {
  server?.kill('SIGTERM');
  models.server.close();
  removeContainer();
}

process.on('SIGTERM', () => {
  stop();
  process.exit(0);
});
process.on('SIGINT', () => {
  stop();
  process.exit(0);
});

startDatabase();
// Offline query cache only: a live DATABASE_URL must never reach the sqlx macros.
run('cargo', ['build', '--quiet', '--locked', '-p', 'vak-server'], { SQLX_OFFLINE: 'true' });
const binary = resolve(backend, 'target', 'debug', 'vak-server');
// The developer's own VAK_* settings (models, paths) must not leak into the e2e server.
const inherited = Object.fromEntries(
  Object.entries(process.env).filter(([name]) => !name.startsWith('VAK_')),
);
const env = {
  ...inherited,
  DATABASE_URL: databaseUrl,
  VAK_PROFILE: 'local',
  VAK_DEV_OPERATOR_EMAIL: DEV_OPERATOR,
  // Purposes' HTTP tools may name these hosts (nothing is ever called: tools run in M4b).
  VAK_TOOL_HOSTS: 'calendar.example.com,*.tools.example.com',
  VAK_HTTP_ADDR: `127.0.0.1:${ports.api}`,
  VAK_LOG: process.env['VAK_LOG'] ?? 'warn',
  VAK_LLM_BASE_URL: `${models.baseUrl}/v1`,
  VAK_LLM_MODEL: FAKE_LLM_MODEL,
  VAK_DECISION_BASE_URL: models.baseUrl,
  VAK_DECISION_MODEL: FAKE_DECISION_MODEL,
};
execFileSync(binary, ['migrate'], { stdio: 'inherit', env });
server = spawn(binary, ['serve'], { stdio: 'inherit', env });
server.on('exit', (code) => {
  models.server.close();
  removeContainer();
  process.exit(code ?? 1);
});
