import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';

import { ApiError, ContractViolationError, http } from './http';

function respond(status: number, body?: unknown, contentType = 'application/json') {
  const text = body === undefined ? '' : typeof body === 'string' ? body : JSON.stringify(body);
  const fetchMock = vi.fn<typeof fetch>(async () =>
    Promise.resolve(
      new Response(status === 204 ? null : text, {
        status,
        headers: { 'content-type': contentType },
      }),
    ),
  );
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

const agent = z.object({ id: z.string(), name: z.string() });

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('http', () => {
  it('returns the parsed body when it matches the schema', async () => {
    respond(200, { id: 'a1', name: 'Support' });
    await expect(http('/api/agents/a1', { schema: agent })).resolves.toEqual({
      id: 'a1',
      name: 'Support',
    });
  });

  it('asks for JSON and sends the same-origin session cookie', async () => {
    const fetchMock = respond(204);
    await http('/api/agents/a1', { method: 'DELETE' });
    const init = fetchMock.mock.calls[0]?.[1];
    expect(init?.credentials).toBe('same-origin');
    expect(new Headers(init?.headers).get('accept')).toContain('application/json');
  });

  it('keeps an Accept header the caller set', async () => {
    const fetchMock = respond(200, 'ok', 'text/plain');
    await expect(http('/api/health', { headers: { accept: 'text/plain' } })).resolves.toBe('ok');
    expect(new Headers(fetchMock.mock.calls[0]?.[1]?.headers).get('accept')).toBe('text/plain');
  });

  it('rejects a 2xx body that breaks the contract', async () => {
    respond(200, { id: 42 });
    await expect(http('/api/agents/a1', { schema: agent })).rejects.toBeInstanceOf(
      ContractViolationError,
    );
  });

  it('turns problem+json into an ApiError', async () => {
    respond(
      422,
      { type: 'about:blank', title: 'Invalid agent', detail: 'name is required' },
      'application/problem+json',
    );
    const error = await http('/api/agents', { method: 'POST' }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({
      status: 422,
      message: 'name is required',
      problem: { title: 'Invalid agent', status: 422 },
    });
  });

  it('keeps a plain-text error body as the problem detail', async () => {
    respond(502, 'Bad gateway', 'text/plain');
    await expect(http('/api/agents')).rejects.toMatchObject({
      status: 502,
      problem: { status: 502, detail: 'Bad gateway' },
    });
  });

  it('describes an error without a body by its status', async () => {
    respond(500);
    await expect(http('/api/agents')).rejects.toThrow('Request failed with status 500');
  });
});
