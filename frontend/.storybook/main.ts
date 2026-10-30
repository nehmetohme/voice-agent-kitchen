import type { StorybookConfig } from '@storybook/react-vite';

// Minimal Storybook for shared/ui. Stories sit next to their component (`*.stories.tsx`).
// The a11y and Vitest addons come later, when stories start doubling as tests.
const config: StorybookConfig = {
  framework: '@storybook/react-vite',
  stories: ['../src/shared/ui/**/*.stories.tsx'],
  // TypeScript 7 has no compiler API, so prop docs use the Babel-based react-docgen.
  typescript: { reactDocgen: 'react-docgen' },
  core: { disableTelemetry: true },
};

export default config;
