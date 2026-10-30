// The orval client generated from contracts/openapi.json goes through http.ts, so every
// response is parsed with the contract's Zod schema. Its MSW handlers mock the same contract.
import { HttpResponse, http as mswHttp } from 'msw';
import { setupServer } from 'msw/node';
import { afterAll, afterEach, beforeAll, describe, expect, it } from 'vitest';

import { getHealth } from './generated/endpoints/health/health';
import { getGetHealthMockHandler } from './generated/endpoints/health/health.msw';
import { ContractViolationError } from './http';

const server = setupServer();

beforeAll(() => {
  server.listen({ onUnhandledRequest: 'error' });
});
afterEach(() => {
  server.resetHandlers();
});
afterAll(() => {
  server.close();
});

describe('generated API client', () => {
  it('fetches GET /health and returns the parsed body', async () => {
    server.use(getGetHealthMockHandler({ status: 'ok', version: '0.1.0' }));

    await expect(getHealth()).resolves.toEqual({ status: 'ok', version: '0.1.0' });
  });

  it('ships mocks that satisfy the contract', async () => {
    server.use(getGetHealthMockHandler());

    const health = await getHealth();

    expect(health.status).toBe('ok');
    expect(typeof health.version).toBe('string');
  });

  it('rejects a response that breaks the contract', async () => {
    server.use(mswHttp.get('*/health', () => HttpResponse.json({ status: 'down', version: 1 })));

    await expect(getHealth()).rejects.toBeInstanceOf(ContractViolationError);
  });
});
