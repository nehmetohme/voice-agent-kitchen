// The agents feature's hooks against the in-memory API fake: queries, mutations, the voice
// choice and autosave (with a short delay so the test runs on real timers).
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { http } from 'msw';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { AGENT_ID, agentBody, fakeAgentsApi, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import type { AgentDetails } from '../api/agents-api';
import { useArchiveAgent, useCreateAgent } from './agent-mutations';
import { agentKeys } from './agents-queries';
import { useAutosave } from './use-autosave';
import { useVoiceChoice } from './use-voice-choice';

const api = fakeAgentsApi([agentBody()]);
const server = startMswServer(...api.handlers);

function wrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { queryClient, Wrapper };
}

const agent: AgentDetails = {
  id: AGENT_ID,
  archived: false,
  settings: {
    name: 'Front desk',
    useCase: 'support',
    voice: 'af_heart',
    language: 'en-US',
    jurisdiction: 'US',
    model: 'balanced',
    turnDetection: { silenceMs: 500, bargeIn: true },
  },
};

describe('useVoiceChoice', () => {
  it('preselects the recommended voice until another is chosen', async () => {
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useVoiceChoice(), { wrapper: Wrapper });

    expect(result.current.loading).toBe(true);
    await waitFor(() => {
      expect(result.current.value).toBe('af_heart');
    });
    act(() => {
      result.current.choose('bm_george');
    });
    expect(result.current.value).toBe('bm_george');
    expect(result.current.options).toHaveLength(2);
  });
});

describe('mutations', () => {
  it('creating seeds the new agent in the cache', async () => {
    const { Wrapper, queryClient } = wrapper();
    const { result } = renderHook(() => useCreateAgent(), { wrapper: Wrapper });

    const created = await result.current.mutateAsync({
      name: 'Bookings',
      useCase: 'booking',
      voice: 'bm_george',
    });

    expect(created.settings.name).toBe('Bookings');
    expect(queryClient.getQueryData(agentKeys.detail(created.id))).toEqual(created);
  });

  it('archiving invalidates every agent query', async () => {
    const { Wrapper, queryClient } = wrapper();
    queryClient.setQueryData(agentKeys.list(), []);
    const { result } = renderHook(() => useArchiveAgent(AGENT_ID), { wrapper: Wrapper });

    await result.current.mutateAsync();

    expect(queryClient.getQueryState(agentKeys.list())?.isInvalidated).toBe(true);
  });
});

describe('useAutosave', () => {
  it('saves only the changed field after the quiet time, then says saved', async () => {
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useAutosave(agent, 10), { wrapper: Wrapper });
    expect(result.current.status).toBe('saved');

    act(() => {
      result.current.edit('name', 'After hours');
    });

    expect(result.current.status).toBe('unsaved');
    await waitFor(() => {
      expect(result.current.status).toBe('saved');
    });
    expect(api.patches.at(-1)).toEqual({ name: 'After hours' });
    expect(result.current.draft.name).toBe('After hours');
  });

  it('never sends an invalid field', async () => {
    const { Wrapper } = wrapper();
    const sent = api.patches.length;
    const { result } = renderHook(() => useAutosave(agent, 10), { wrapper: Wrapper });

    act(() => {
      result.current.edit('name', '  ');
    });
    await new Promise((resolve) => {
      setTimeout(resolve, 50);
    });

    expect(result.current.status).toBe('invalid');
    expect(result.current.errors.name).toBe('Enter a name.');
    expect(api.patches).toHaveLength(sent);
  });

  it('reports a failed save with the server field errors, and retries on request', async () => {
    const { Wrapper } = wrapper();
    server.use(
      http.patch('*/api/v1/agents/:id', () =>
        problemResponse(422, [{ field: 'voice', message: 'Choose a voice from the catalog.' }]),
      ),
    );
    const { result } = renderHook(() => useAutosave(agent, 10), { wrapper: Wrapper });

    act(() => {
      result.current.edit('voice', 'robot');
    });
    await waitFor(() => {
      expect(result.current.status).toBe('failed');
    });
    expect(result.current.errors.voice).toBe('Choose a voice from the catalog.');

    server.resetHandlers();
    act(() => {
      result.current.retry();
    });
    await waitFor(() => {
      expect(result.current.status).toBe('saved');
    });
  });
});
