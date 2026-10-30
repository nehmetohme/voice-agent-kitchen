// One MSW server per test file: listens before the tests, resets overrides after each one,
// and fails any request no handler answers.
import type { RequestHandler } from 'msw';
import { setupServer } from 'msw/node';
import { afterAll, afterEach, beforeAll } from 'vitest';

export function startMswServer(...handlers: RequestHandler[]) {
  const server = setupServer(...handlers);
  beforeAll(() => {
    server.listen({ onUnhandledFrame: 'error' });
  });
  afterEach(() => {
    server.resetHandlers();
  });
  afterAll(() => {
    server.close();
  });
  return server;
}
