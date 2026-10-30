import tailwindcss from '@tailwindcss/vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The Rust server serves /api in every environment; the dev and preview servers forward to
// it. VAK_API_URL moves it (the E2E run uses its own port per worktree).
const apiTarget = process.env['VAK_API_URL'] ?? 'http://127.0.0.1:3000';

export default defineConfig({
  plugins: [
    // Must run before the React plugin. Generates src/app/routeTree.gen.ts from src/routes.
    tanstackRouter({
      target: 'react',
      routesDirectory: './src/routes',
      generatedRouteTree: './src/app/routeTree.gen.ts',
      autoCodeSplitting: true,
    }),
    react(),
    tailwindcss(),
  ],
  server: {
    port: 5173,
    strictPort: true,
    // `vite preview` reuses this proxy.
    proxy: { '/api': { target: apiTarget, ws: true } },
  },
  build: {
    target: 'es2023',
    sourcemap: true,
  },
});
