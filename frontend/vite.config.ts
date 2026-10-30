import tailwindcss from '@tailwindcss/vite';
import { tanstackRouter } from '@tanstack/router-plugin/vite';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

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
    // The Rust server serves /api in every environment; the dev server forwards to it.
    proxy: { '/api': { target: 'http://127.0.0.1:3000', ws: true } },
  },
  build: {
    target: 'es2023',
    sourcemap: true,
  },
});
