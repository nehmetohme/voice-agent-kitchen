import { defineConfig, mergeConfig } from 'vitest/config';

import viteConfig from './vite.config.ts';

// Coverage floors come from docs/plan/quality.md §1 (line coverage).
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      projects: [
        {
          extends: true,
          test: {
            name: 'dom',
            environment: 'jsdom',
            include: ['src/**/*.test.{ts,tsx}'],
            setupFiles: ['./src/test/setup.ts'],
          },
        },
        {
          extends: true,
          test: {
            name: 'node',
            environment: 'node',
            include: ['tools/**/*.test.ts', 'tokens/**/*.test.ts', 'e2e/support/**/*.test.ts'],
          },
        },
      ],
      coverage: {
        provider: 'v8',
        include: ['src/**/*.{ts,tsx}', 'tools/**/*.ts', 'tokens/**/*.ts'],
        exclude: [
          '**/*.test.{ts,tsx}',
          '**/*.stories.tsx',
          'src/test/**',
          'src/app/main.tsx',
          'src/app/routeTree.gen.ts',
          'src/shared/api/generated/**',
        ],
        reporter: ['text-summary', 'html', 'lcov'],
        thresholds: {
          lines: 80,
          'src/features/*/state/**': { lines: 90 },
          'src/shared/lib/**': { lines: 90 },
        },
      },
    },
  }),
);
