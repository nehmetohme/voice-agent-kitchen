import { defineConfig } from 'orval';

// Generates the typed API client from the backend contract (contracts/openapi.json, written
// from Rust by `vak-server openapi`). Run it with `just openapi` (spec and client) or
// `pnpm api:generate` (client only); never hand-edit src/shared/api/generated/.
// `just contracts-verify` regenerates it and fails if the committed output differs.
//
// The generated *.msw.ts mocks use the devDependencies `msw` and `@faker-js/faker`;
// dependency-cruiser treats them as test code.
export default defineConfig({
  vak: {
    input: { target: '../contracts/openapi.json' },
    output: {
      mode: 'tags-split',
      target: 'src/shared/api/generated/endpoints',
      schemas: { path: 'src/shared/api/generated/model', type: 'zod' },
      client: 'react-query',
      httpClient: 'fetch',
      clean: true,
      mock: { generators: [{ type: 'msw' }] },
      override: {
        // Every request goes through http.ts, which also Zod-parses every response.
        mutator: { path: 'src/shared/api/http.ts', name: 'http' },
        includeZodSchemaInArguments: true,
        fetch: { runtimeValidation: true, includeHttpResponseReturnType: false },
        query: { useQuery: true },
      },
    },
  },
});
