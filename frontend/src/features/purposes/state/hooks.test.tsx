// The purposes queries and the autosaving draft, against the in-memory API fake.
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { http } from 'msw';
import type { ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { problemResponse, SYSTEM_PURPOSE_IDS } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { CUSTOM_ID, fakePurposesApi, purposeBody } from '../../../test/purposes-api';
import { getPurpose } from '../api/purposes-api';
import {
  purposeQueryOptions,
  purposesQueryOptions,
  toolCatalogQueryOptions,
  useDuplicatePurpose,
  useRefreshPurpose,
  useSaveVersion,
} from './purposes-queries';
import { usePurposeDraft } from './use-purpose-draft';

const server = startMswServer();

function setup() {
  const api = fakePurposesApi([purposeBody()]);
  server.use(...api.handlers);
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { api, queryClient, wrapper };
}

describe('purposes queries', () => {
  it('lists purposes, reads one and the tool catalog', async () => {
    const { wrapper } = setup();

    const list = renderHook(() => useQuery(purposesQueryOptions()), { wrapper });
    const one = renderHook(() => useQuery(purposeQueryOptions(CUSTOM_ID)), { wrapper });
    const tools = renderHook(() => useQuery(toolCatalogQueryOptions()), { wrapper });

    await waitFor(() => {
      expect(tools.result.current.data?.allowedHosts).toEqual([
        'calendar.example.com',
        '*.tools.test',
      ]);
    });
    await waitFor(() => {
      expect(list.result.current.data?.map((item) => item.name)).toEqual([
        'Customer support',
        'Appointment booking',
        'Clinic bookings',
      ]);
    });
    await waitFor(() => {
      expect(one.result.current.data?.settings.overview.complianceClass).toBe('booking');
    });
  });

  it('caches a copy so its page opens without a fetch', async () => {
    const { queryClient, wrapper } = setup();
    const hook = renderHook(() => useDuplicatePurpose(), { wrapper });

    act(() => {
      hook.result.current.mutate({ source: SYSTEM_PURPOSE_IDS.booking, name: 'Dental' });
    });

    await waitFor(() => {
      expect(hook.result.current.isSuccess).toBe(true);
    });
    const cached = queryClient.getQueryData(purposeQueryOptions(CUSTOM_ID).queryKey);
    expect(cached?.settings.overview.name).toBe('Dental');
  });

  it('saves a version and refreshes on demand', async () => {
    const { queryClient, wrapper } = setup();
    const save = renderHook(() => useSaveVersion(CUSTOM_ID), { wrapper });
    const refresh = renderHook(() => useRefreshPurpose(CUSTOM_ID), { wrapper });

    act(() => {
      save.result.current.mutate();
    });

    await waitFor(() => {
      expect(save.result.current.data?.latestVersion).toBe(1);
    });
    expect(queryClient.getQueryData(purposeQueryOptions(CUSTOM_ID).queryKey)?.unsavedChanges).toBe(
      false,
    );
    act(() => {
      refresh.result.current();
    });
    expect(queryClient.getQueryState(purposeQueryOptions(CUSTOM_ID).queryKey)?.isInvalidated).toBe(
      true,
    );
  });
});

async function draftHook(id: string = CUSTOM_ID) {
  const context = setup();
  const start = await getPurpose(id);
  const hook = renderHook(() => usePurposeDraft(start, 10), { wrapper: context.wrapper });
  return { ...context, start, hook };
}

describe('usePurposeDraft', () => {
  it('sends nothing until the operator edits', async () => {
    const { api, hook } = await draftHook();

    expect(hook.result.current.status).toBe('saved');
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(api.patches).toEqual([]);
  });

  it('autosaves only the changed section', async () => {
    const { api, hook, start } = await draftHook();
    const guardrails = { ...start.settings.guardrails, allowedTopics: ['Appointments'] };

    act(() => {
      hook.result.current.edit('guardrails', guardrails);
    });

    expect(hook.result.current.dirty).toBe(true);
    await waitFor(() => {
      expect(hook.result.current.status).toBe('saved');
    });
    expect(api.patches).toHaveLength(1);
    expect(Object.keys(api.patches[0] ?? {})).toEqual(['guardrails']);
    expect(hook.result.current.draft.guardrails.allowedTopics).toEqual(['Appointments']);
  });

  it('holds a change the contract refuses without sending it', async () => {
    const { api, hook, start } = await draftHook();

    act(() => {
      hook.result.current.edit('overview', { ...start.settings.overview, name: '' });
    });

    expect(hook.result.current.status).toBe('invalid');
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(api.patches).toEqual([]);
  });

  it('shows the server field errors after a refused save, and retries', async () => {
    const { api, hook, start } = await draftHook();
    server.use(
      http.patch('*/api/v1/purposes/:id', () =>
        problemResponse(422, [{ field: 'tools.http[0].url', message: 'Use https.' }]),
      ),
    );

    act(() => {
      hook.result.current.edit('overview', { ...start.settings.overview, name: 'Dental' });
    });

    await waitFor(() => {
      expect(hook.result.current.status).toBe('failed');
    });
    expect(hook.result.current.errors).toEqual([
      { field: 'tools.http[0].url', message: 'Use https.' },
    ]);
    server.use(...api.handlers);
    act(() => {
      hook.result.current.retry();
    });
    await waitFor(() => {
      expect(hook.result.current.status).toBe('saved');
    });
    expect(hook.result.current.errors).toEqual([]);
  });

  it('never sends a built-in purpose', async () => {
    const { api, hook, start } = await draftHook(SYSTEM_PURPOSE_IDS.booking);

    act(() => {
      hook.result.current.edit('overview', { ...start.settings.overview, name: 'Mine' });
    });

    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(api.patches).toEqual([]);
    expect(hook.result.current.status).toBe('unsaved');
  });
});
