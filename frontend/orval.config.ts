import { defineConfig } from 'orval';

// Generates the typed API client from the backend contract. Run it with `just openapi`
// (or `pnpm api:generate`), never by hand-editing src/shared/api/generated/.
//
// contracts/openapi.json does not exist yet (task T0.4 emits it from Rust), so `verify`
// does not run this. The generated mocks need `msw` and `@faker-js/faker`; T0.4 adds them
// together with the first generated client.
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
