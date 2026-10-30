import type { KnipConfig } from 'knip';

// knip finds unused files, exports and dependencies. Its plugins already pick up the entry
// points named in vite (index.html), vitest, playwright, storybook, oxlint (jsPlugins),
// orval (the mutator) and package.json scripts.
const config: KnipConfig = {
  entry: [
    // Each feature's public API. Placeholders stay until their milestone fills them.
    'src/features/*/index.ts',
  ],
  project: [
    'src/**/*.{ts,tsx,css}',
    'tools/**/*.ts',
    'tokens/**/*.ts',
    'e2e/**/*.ts',
    '.storybook/**/*.ts',
  ],
  // The router plugin and orval write these; they are never edited by hand.
  ignore: ['src/app/routeTree.gen.ts', 'src/shared/api/generated/**'],
  // dependency-cruiser loads @swc/core by name (`parser: 'swc'` in .dependency-cruiser.cjs).
  ignoreDependencies: ['@swc/core'],
  compilers: {
    // Follow `@import` in stylesheets, so `tailwindcss` counts as used.
    css: (text: string) =>
      [...text.matchAll(/@import\s+['"]([^'"]+)['"]/g)]
        .map(([, specifier]) => `import '${specifier ?? ''}';`)
        .join('\n'),
  },
};

export default config;
