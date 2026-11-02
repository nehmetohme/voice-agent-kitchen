import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import { http } from 'msw';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { AGENT_ID, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { fakePlansApi } from '../../../test/plans-api';
import { getPlan, type PlanView } from '../api/plans-api';
import { planQueryOptions } from './plans-queries';
import { usePlanEditor } from './use-plan-editor';

const server = startMswServer();

function setup() {
  const api = fakePlansApi(AGENT_ID);
  server.use(...api.handlers);
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  return { api, queryClient, wrapper };
}

async function editor(initial?: PlanView) {
  const context = setup();
  const start = initial ?? (await getPlan(AGENT_ID));
  const onSaved = vi.fn<() => void>();
  const hook = renderHook(() => usePlanEditor(AGENT_ID, start, { onSaved, delayMs: 10 }), {
    wrapper: context.wrapper,
  });
  return { ...context, hook, onSaved };
}

describe('usePlanEditor', () => {
  it('shows the template unsaved and sends nothing until the operator acts', async () => {
    const { api, hook } = await editor();

    expect(hook.result.current.view.saved).toBe(false);
    expect(hook.result.current.status).toBe('unsaved');
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(api.saves).toEqual([]);
  });

  it('saves the template as it is on "Use this plan"', async () => {
    const { api, hook, onSaved, queryClient } = await editor();

    act(() => {
      hook.result.current.keepTemplate();
    });

    await waitFor(() => {
      expect(hook.result.current.status).toBe('saved');
    });
    expect(hook.result.current.view.saved).toBe(true);
    expect(api.saves).toHaveLength(1);
    expect(onSaved).toHaveBeenCalledOnce();
    expect(queryClient.getQueryData(planQueryOptions(AGENT_ID).queryKey)?.saved).toBe(true);
  });

  it('autosaves each edit and shows the problems the server finds', async () => {
    const { api, hook } = await editor();

    act(() => {
      hook.result.current.addStep({ label: 'Wrap up', kind: 'say' });
    });
    expect(hook.result.current.status).toBe('unsaved');
    await waitFor(() => {
      expect(hook.result.current.view.problems).toEqual([
        expect.objectContaining({ code: 'missing_exit', stepId: 'step-1' }),
      ]);
    });

    act(() => {
      hook.result.current.addPath({ from: 'step-1', to: 'goodbye', condition: 'Done' });
      hook.result.current.updateStep('step-1', { text: 'Wrap up the call.' });
    });
    await waitFor(() => {
      expect(hook.result.current.view.valid).toBe(true);
    });
    act(() => {
      hook.result.current.deletePath('path-1');
      hook.result.current.deleteStep('step-1');
    });
    await waitFor(() => {
      expect(hook.result.current.status).toBe('saved');
    });
    expect(api.saves.length).toBeGreaterThanOrEqual(2);
    expect(hook.result.current.draft.steps.map((step) => step.id)).toEqual([
      'start',
      'ask',
      'goodbye',
    ]);
  });

  it('holds a draft the contract rejects, and retries after a failed save', async () => {
    const { hook } = await editor();

    act(() => {
      hook.result.current.updateStep('ask', { label: '' });
    });
    expect(hook.result.current.status).toBe('invalid');

    server.use(http.put('*/api/v1/agents/:id/plan', () => problemResponse(503)));
    act(() => {
      hook.result.current.updateStep('ask', { label: 'Ask' });
    });
    await waitFor(() => {
      expect(hook.result.current.status).toBe('failed');
    });

    server.resetHandlers(...setup().api.handlers);
    act(() => {
      hook.result.current.retry();
    });
    await waitFor(() => {
      expect(hook.result.current.status).toBe('saved');
    });
  });

  it('loads the plan through its query options', async () => {
    const { queryClient } = setup();

    const plan = await queryClient.query(planQueryOptions(AGENT_ID));

    expect(plan.steps).toHaveLength(3);
  });
});
