// The Plan step through the user's eyes: roles and names, against the in-memory plans fake.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { http } from 'msw';
import { describe, expect, it } from 'vitest';

import { AGENT_ID, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { fakePlansApi, type FakeProblem } from '../../../test/plans-api';
import { renderRoutes } from '../../../test/render-routes';
import { PlanPage } from './plan-page';

const server = startMswServer();

function PlanAtId() {
  return <PlanPage agentId={AGENT_ID} />;
}

function open(validate?: Parameters<typeof fakePlansApi>[1]) {
  const api = fakePlansApi(AGENT_ID, validate);
  server.use(...api.handlers);
  renderRoutes(`/agents/${AGENT_ID}/plan`, [
    { path: '/agents/$agentId/plan', component: PlanAtId },
    { path: '/agents', component: () => <p>Agents</p> },
  ]);
  return api;
}

const step = (name: string) => screen.findByRole('group', { name });

async function addStep(name: string) {
  fireEvent.click(screen.getByRole('button', { name: 'Add step' }));
  const dialog = await screen.findByRole('dialog', { name: 'Add step' });
  fireEvent.change(within(dialog).getByLabelText('Name'), { target: { value: name } });
  fireEvent.change(within(dialog).getByLabelText('Kind'), { target: { value: 'say' } });
  fireEvent.click(within(dialog).getByRole('button', { name: 'Add step' }));
}

async function addPath(from: string, to: string, when: string) {
  fireEvent.click(within(await step(from)).getByRole('button', { name: 'Add path' }));
  const dialog = await screen.findByRole('dialog', { name: `Add path from ${from}` });
  fireEvent.change(within(dialog).getByLabelText('To'), { target: { value: to } });
  fireEvent.change(within(dialog).getByLabelText('When'), { target: { value: when } });
  fireEvent.click(within(dialog).getByRole('button', { name: 'Add path' }));
}

describe('Plan page', () => {
  it('shows the template, with the locked opening, until "Use this plan" saves it', async () => {
    const api = open();

    const greeting = await step('Greeting');
    expect(within(greeting).getByText('Locked by the rule pack')).toBeVisible();
    expect(within(greeting).getByText('How can I help?')).toBeVisible();
    expect(within(greeting).getByRole('list', { name: 'Paths' })).toHaveTextContent(
      'The caller answers',
    );
    expect(within(greeting).queryByRole('button', { name: 'Delete step' })).toBeNull();
    expect(await step('Goodbye')).toBeVisible();
    expect(screen.getByRole('status', { name: 'Save status' })).toHaveTextContent(
      'Unsaved changes',
    );
    const use = screen.getByRole('button', { name: 'Use this plan' });
    expect(use).toHaveAttribute('data-variant', 'primary');

    fireEvent.click(use);

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Use this plan' })).toBeNull();
    });
    expect(screen.getByRole('status', { name: 'Save status' })).toHaveTextContent('Saved');
    expect(screen.getByText('The plan follows every rule.')).toBeVisible();
    expect(api.saves).toHaveLength(1);
  });

  it('adds a step that shows its problem until a path leads out of it', async () => {
    open();
    await step('Greeting');

    fireEvent.click(screen.getByRole('button', { name: 'Add step' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add step' });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add step' }));
    expect(within(dialog).getByText('Give the step a name.')).toBeVisible();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await addStep('Wrap up');

    const wrapUp = await step('Wrap up');
    expect(await within(wrapUp).findByText(/has no path out/)).toBeVisible();
    expect(await screen.findByText('1 problem to fix, shown where they are.')).toBeVisible();

    fireEvent.click(within(wrapUp).getByRole('button', { name: 'Add path' }));
    const pathDialog = await screen.findByRole('dialog', { name: 'Add path from Wrap up' });
    fireEvent.click(within(pathDialog).getByRole('button', { name: 'Add path' }));
    expect(within(pathDialog).getByText('Choose the step the path leads to.')).toBeVisible();
    expect(within(pathDialog).getByText('Say when to take this path.')).toBeVisible();
    fireEvent.click(within(pathDialog).getByRole('button', { name: 'Cancel' }));
    await addPath('Wrap up', 'goodbye', 'The caller is done');

    await waitFor(() => {
      expect(within(wrapUp).queryByText(/has no path out/)).toBeNull();
    });
    expect(within(wrapUp).getByRole('list', { name: 'Paths' })).toHaveTextContent(
      'The caller is done',
    );
  });

  it('edits a step, keeps the locked opening in front of the greeting, and removes paths', async () => {
    const api = open();

    fireEvent.click(within(await step('Greeting')).getByRole('button', { name: 'Edit step' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit Greeting' });
    fireEvent.change(within(dialog).getByLabelText('Greeting'), { target: { value: 'Hi there!' } });
    fireEvent.change(within(dialog).getByLabelText('Name'), { target: { value: 'Hello' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save step' }));

    const hello = await step('Hello');
    expect(within(hello).getByText('Hi there!')).toBeVisible();
    fireEvent.click(
      within(hello).getByRole('button', { name: 'Remove path to Ask what they need' }),
    );
    await waitFor(() => {
      expect(api.saves.length).toBeGreaterThan(0);
    });
    const saved = api.saves.at(-1);
    // The greeting follows the locked texts, which stay in front of it.
    expect(JSON.stringify(saved)).toMatch(/This call is recorded and transcribed\. Hi there!"/);
    expect(within(hello).queryByRole('list', { name: 'Paths' })).toBeNull();

    fireEvent.click(
      within(await step('Ask what they need')).getByRole('button', { name: 'Edit step' }),
    );
    const ask = await screen.findByRole('dialog', { name: 'Edit Ask what they need' });
    fireEvent.change(within(ask).getByLabelText('Name'), { target: { value: ' ' } });
    fireEvent.click(within(ask).getByRole('button', { name: 'Save step' }));
    expect(within(ask).getByText('Give the step a name.')).toBeVisible();
  });

  it('deletes a step after a confirmation', async () => {
    open();

    fireEvent.click(
      within(await step('Ask what they need')).getByRole('button', { name: 'Delete step' }),
    );
    const confirm = await screen.findByRole('alertdialog', { name: 'Delete Ask what they need?' });
    fireEvent.click(within(confirm).getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(screen.queryByRole('group', { name: 'Ask what they need' })).toBeNull();
    });
    expect(within(await step('Greeting')).queryByRole('list', { name: 'Paths' })).toBeNull();
  });

  it('shows each problem where it belongs: on a path, on a step, or above the plan', async () => {
    const problems: FakeProblem[] = [
      { code: 'rule_pack_unavailable', message: 'No rule pack covers this agent yet.' },
      { code: 'empty_condition', message: 'This path needs a condition.', edgeId: 'p1' },
      { code: 'unreachable_node', message: 'No path leads here.', nodeId: 'goodbye' },
    ];
    open(() => problems);

    const greeting = await step('Greeting');
    expect(within(greeting).getByText('This path needs a condition.')).toBeVisible();
    expect(within(await step('Goodbye')).getByText('No path leads here.')).toBeVisible();
    expect(screen.getByRole('list', { name: 'Plan problems' })).toHaveTextContent(
      'No rule pack covers this agent yet.',
    );
  });

  it('says when the agent is missing, and offers a retry when loading fails', async () => {
    server.use(http.get('*/api/v1/agents/:id/plan', () => problemResponse(404)));
    renderRoutes(`/agents/${AGENT_ID}/plan`, [
      { path: '/agents/$agentId/plan', component: PlanAtId },
    ]);
    expect(await screen.findByRole('heading', { name: "This agent doesn't exist" })).toBeVisible();
  });

  it('shows busy feedback at once, and a retry when loading fails', async () => {
    server.use(http.get('*/api/v1/agents/:id/plan', () => problemResponse(503)));
    renderRoutes(`/agents/${AGENT_ID}/plan`, [
      { path: '/agents/$agentId/plan', component: PlanAtId },
    ]);

    expect(await screen.findByText('Loading the plan…')).toHaveAttribute('aria-busy', 'true');
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't load the plan");
    server.use(...fakePlansApi(AGENT_ID).handlers);
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await step('Greeting')).toBeVisible();
  });
});
