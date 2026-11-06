// A purpose's starting flow in the flow editor: edited and autosaved like a plan, or only
// shown when the purpose is built-in.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { http } from 'msw';
import { describe, expect, it, vi } from 'vitest';

import { problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { fakePurposeFlowApi, LOCKED_TEXTS } from '../../../test/plans-api';
import { renderRoutes } from '../../../test/render-routes';
import { PurposeFlow } from './purpose-flow';

const server = startMswServer();
const PURPOSE = '019a5a5a-0000-7000-8000-000000000001';

function open(readOnly: boolean, onSaved?: () => void) {
  const api = fakePurposeFlowApi(PURPOSE);
  server.use(...api.handlers);
  renderRoutes('/purposes/flow', [
    {
      path: '/purposes/flow',
      component: () => <PurposeFlow purposeId={PURPOSE} readOnly={readOnly} onSaved={onSaved} />,
    },
  ]);
  return api;
}

describe('PurposeFlow', () => {
  it('shows the flow after the locked texts every agent opens with, and autosaves an edit', async () => {
    const onSaved = vi.fn<() => void>();
    const api = open(false, onSaved);

    const greeting = await screen.findByRole('group', { name: 'Greeting' });
    expect(greeting).toHaveTextContent('How can I help?');
    expect(
      screen.getByText(/Every agent's opening starts with its rule pack's locked texts/),
    ).toBeVisible();
    expect(screen.getByText(LOCKED_TEXTS.map(({ text }) => text).join(' '))).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Use this plan' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Add step' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add step' });
    fireEvent.change(within(dialog).getByLabelText('Name'), { target: { value: 'Wrap up' } });
    fireEvent.change(within(dialog).getByLabelText('Kind'), { target: { value: 'end' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add step' }));

    await waitFor(
      () => {
        expect(api.saves).toHaveLength(1);
      },
      { timeout: 2000 },
    );
    await waitFor(() => {
      expect(onSaved).toHaveBeenCalledOnce();
    });
    expect(await screen.findByRole('group', { name: 'Wrap up' })).toBeVisible();
  });

  it('only shows a built-in purpose flow', async () => {
    open(true);

    const greeting = await screen.findByRole('group', { name: 'Greeting' });
    expect(within(greeting).queryByRole('button')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Add step' })).not.toBeInTheDocument();
    expect(screen.queryByRole('status', { name: 'Save status' })).not.toBeInTheDocument();
  });

  it('offers a retry when the flow fails to load', async () => {
    const api = open(false);
    server.use(http.get('*/api/v1/purposes/:id/flow', () => problemResponse(503), { once: true }));

    expect(await screen.findByRole('alert')).toBeVisible();
    server.use(...api.handlers);
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('group', { name: 'Greeting' })).toBeVisible();
  });
});
