// The agents screens through the user's eyes: roles and names, against the in-memory API fake.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { HttpResponse, http } from 'msw';
import { describe, expect, it } from 'vitest';

import { AGENT_ID, agentBody, fakeAgentsApi, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { renderRoutes } from '../../../test/render-routes';
import { AgentsOverview } from './agents-overview';
import { CreateAgentPage } from './create-agent-page';
import { ProvisionPage } from './provision-page';

const server = startMswServer();

function ProvisionAtId() {
  return <ProvisionPage agentId={AGENT_ID} />;
}

const ROUTES = [
  { path: '/agents', component: AgentsOverview },
  { path: '/agents/new', component: CreateAgentPage },
  { path: '/agents/$agentId/provision', component: ProvisionAtId },
];

function use(agents = [agentBody()]) {
  const api = fakeAgentsApi(agents);
  server.use(...api.handlers);
  return api;
}

describe('Agents list', () => {
  it('offers one primary action, Create agent, when there are no agents', async () => {
    use([]);
    renderRoutes('/agents', ROUTES);

    expect(await screen.findByRole('heading', { name: 'No agents yet' })).toBeVisible();
    const create = screen.getByRole('link', { name: 'Create agent' });
    expect(create).toHaveAttribute('data-variant', 'primary');
    expect(create).toHaveAttribute('href', '/agents/new');
  });

  it('lists agents with their purpose and setup progress', async () => {
    use();
    renderRoutes('/agents', ROUTES);

    const list = await screen.findByRole('list', { name: 'Agents' });
    expect(within(list).getByRole('link', { name: 'Front desk' })).toHaveAttribute(
      'href',
      `/agents/${AGENT_ID}/provision`,
    );
    expect(within(list).getByText('Customer support')).toBeVisible();
    expect(within(list).getByText('25 % set up')).toBeVisible();
  });

  it('shows busy feedback at once, and a retry when loading fails', async () => {
    server.use(http.get('*/api/v1/agents', () => problemResponse(503)));
    renderRoutes('/agents', ROUTES);

    expect(await screen.findByText('Loading agents…')).toHaveAttribute('aria-busy', 'true');
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't load your agents");
    use();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('link', { name: 'Front desk' })).toBeVisible();
  });
});

describe('Create agent', () => {
  it('asks for three fields and opens the new agent', async () => {
    const api = use([]);
    const { router } = renderRoutes('/agents/new', ROUTES);
    const form = await screen.findByRole('form', { name: 'Create agent' });

    expect(within(form).getAllByRole('textbox')).toHaveLength(1);
    expect(within(form).getAllByRole('combobox')).toHaveLength(2);
    await waitFor(() => {
      expect(within(form).getByLabelText('Voice')).toHaveValue('af_heart');
    });
    fireEvent.change(within(form).getByLabelText('Name'), { target: { value: 'Bookings' } });
    fireEvent.change(within(form).getByLabelText('Purpose'), { target: { value: 'booking' } });
    fireEvent.click(within(form).getByRole('button', { name: 'Create agent' }));

    await waitFor(() => {
      expect(router.state.location.pathname).toMatch(/^\/agents\/.+\/provision$/);
    });
    expect([...api.agents.values()].map((agent) => agent.name)).toEqual(['Bookings']);
  });

  it('names each missing field instead of sending', async () => {
    use([]);
    renderRoutes('/agents/new', ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'Create agent' }));

    expect(await screen.findByText('Enter a name.')).toBeVisible();
    expect(screen.getByText('Choose a purpose.')).toBeVisible();
    expect(screen.getByLabelText('Name')).toHaveAttribute('aria-invalid', 'true');
  });

  it('says so when the agent could not be created', async () => {
    use([]);
    server.use(http.post('*/api/v1/agents', () => problemResponse(503)));
    renderRoutes('/agents/new', ROUTES);
    await waitFor(() => {
      expect(screen.getByLabelText('Voice')).toHaveValue('af_heart');
    });

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Bookings' } });
    fireEvent.change(screen.getByLabelText('Purpose'), { target: { value: 'booking' } });
    fireEvent.click(await screen.findByRole('button', { name: 'Create agent' }));

    expect(await screen.findByRole('alert')).toHaveTextContent("The agent wasn't created");
  });
});

describe('Provision', () => {
  it('shows setup progress at 25 % and the settings, saved', async () => {
    use();
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    const progress = await screen.findByRole('progressbar', { name: 'Setup progress' });
    expect(progress).toHaveAttribute('aria-valuenow', '25');
    const steps = within(screen.getByRole('list', { name: 'Setup steps' })).getAllByRole(
      'listitem',
    );
    expect(steps.map((step) => step.textContent)).toEqual([
      'ProvisionDone: Name, purpose and voice',
      'PlanNext: Goals, flow and guardrails',
      'TestLater: Talk to the agent and run scenarios',
      'PublishLater: Release a version to a channel',
    ]);
    expect(screen.getByRole('status')).toHaveTextContent('Saved');
    expect(screen.getByLabelText('Name')).toHaveValue('Front desk');
    expect(screen.queryByRole('button', { name: 'Save' })).toBeNull();
  });

  it('autosaves an edit', async () => {
    const api = use();
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);
    const name = await screen.findByLabelText('Name');

    fireEvent.change(name, { target: { value: 'After hours' } });

    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    await waitFor(
      () => {
        expect(screen.getByRole('status')).toHaveTextContent('Saved');
      },
      { timeout: 2_000 },
    );
    expect(api.patches).toEqual([{ name: 'After hours' }]);
  });

  it('edits the advanced settings', async () => {
    const api = use();
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);
    fireEvent.change(await screen.findByLabelText('Model'), { target: { value: 'fast' } });
    fireEvent.change(screen.getByLabelText('Pause before replying (ms)'), {
      target: { value: '800' },
    });
    fireEvent.click(screen.getByLabelText('Callers can interrupt the agent'));

    await waitFor(
      () => {
        expect(api.agents.get(AGENT_ID)?.turnDetection).toEqual({ silenceMs: 800, bargeIn: false });
      },
      { timeout: 2_000 },
    );
    expect(api.agents.get(AGENT_ID)?.model).toBe('fast');
  });

  it('archives after confirmation and returns to the list', async () => {
    const api = use();
    const { router } = renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'Archive agent' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Archive Front desk?' });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Archive' }));

    await waitFor(() => {
      expect(router.state.location.pathname).toBe('/agents');
    });
    expect(api.agents.get(AGENT_ID)?.archivedAt).not.toBeNull();
  });

  it('offers a way back for an unknown or archived agent', async () => {
    use([agentBody({ archivedAt: '2026-10-30T11:00:00Z' })]);
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    expect(await screen.findByRole('heading', { name: "This agent doesn't exist" })).toBeVisible();
    expect(screen.getByRole('link', { name: 'Go to Agents' })).toHaveAttribute('href', '/agents');
  });

  it('offers a retry when the agent or its progress fails to load', async () => {
    use();
    server.use(
      http.get('*/api/v1/agents/:id/progress', () => problemResponse(503)),
      http.get('*/api/v1/agents/:id', () => HttpResponse.error()),
    );
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't load this agent");
  });
});
