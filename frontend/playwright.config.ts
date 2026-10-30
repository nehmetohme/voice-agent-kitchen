import { resolve } from 'node:path';

import { defineConfig, devices } from '@playwright/test';

import { e2ePorts } from './e2e/support/ports';

// Tests run against the production build (`vite preview`), so build first: `pnpm build`.
// `/api` goes to the real Rust server on a throwaway Postgres (e2e/support/backend-server.ts),
// never to mocks. Parallel worktrees would collide on fixed ports, so each worktree derives
// its own from a hash of its path (e2e/support/ports.ts); E2E_PORT overrides the web port.
const worktree = resolve(import.meta.dirname, '..');
const ports = e2ePorts(worktree);
const baseURL = `http://127.0.0.1:${ports.web}`;
const apiURL = `http://127.0.0.1:${ports.api}`;
const ci = Boolean(process.env['CI']);
const chrome = { ...devices['Desktop Chrome'] };

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 2 : 0,
  reporter: ci ? [['github'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL,
    trace: 'retain-on-failure',
  },
  projects: [
    // Contract specs run first: agent-provisioning starts from an empty agent list, so no
    // other spec may create agents while it runs.
    { name: 'contract', testMatch: 'contract/**/*.spec.ts', use: chrome },
    { name: 'chromium', testIgnore: 'contract/**', dependencies: ['contract'], use: chrome },
  ],
  webServer: [
    {
      // Builds vak-server, starts Postgres in Docker, migrates, serves. The first build of
      // a fresh checkout takes minutes.
      command: 'node e2e/support/backend-server.ts',
      url: `${apiURL}/health`,
      reuseExistingServer: false,
      timeout: 900_000,
      gracefulShutdown: { signal: 'SIGTERM', timeout: 10_000 },
      stdout: 'pipe',
    },
    {
      command: `pnpm exec vite preview --host 127.0.0.1 --port ${ports.web} --strictPort`,
      url: baseURL,
      env: { VAK_API_URL: apiURL },
      // Never reuse: another worktree's server on the same port would be tested by mistake.
      reuseExistingServer: false,
      timeout: 60_000,
    },
  ],
});
