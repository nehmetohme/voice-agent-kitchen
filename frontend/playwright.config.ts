import { resolve } from 'node:path';

import { defineConfig, devices } from '@playwright/test';

import { e2ePorts } from './e2e/support/ports';

// Tests run against the production build (`vite preview`), so build first: `pnpm build`.
// Parallel worktrees would collide on fixed ports, so each worktree derives its own from a
// hash of its path (e2e/support/ports.ts); E2E_PORT overrides the web port.
const worktree = resolve(import.meta.dirname, '..');
const ports = e2ePorts(worktree);
const baseURL = `http://127.0.0.1:${ports.web}`;
const ci = Boolean(process.env['CI']);

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
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: `pnpm exec vite preview --host 127.0.0.1 --port ${ports.web} --strictPort`,
    url: baseURL,
    // Never reuse: another worktree's server on the same port would be tested by mistake.
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
