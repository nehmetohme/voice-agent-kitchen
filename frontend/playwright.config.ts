import { defineConfig, devices } from '@playwright/test';

// Tests run against the production build (`vite preview`), so build first: `pnpm build`.
// Parallel worktrees can collide on the port: set E2E_PORT to move one of them.
const port = Number(process.env['E2E_PORT'] ?? 4173);
const baseURL = `http://127.0.0.1:${port}`;
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
    command: `pnpm exec vite preview --host 127.0.0.1 --port ${port} --strictPort`,
    url: baseURL,
    // Never reuse: another worktree's server on the same port would be tested by mistake.
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
