// The Test step through the user's eyes, against the in-memory test-runs fake: scenarios, a run
// from start to verdict, each result, the stale notice, editing the set, and failures.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { HttpResponse, http } from 'msw';
import { describe, expect, it, vi } from 'vitest';

import { AGENT_ID, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import { renderRoutes } from '../../../test/render-routes';
import { fakeTestRunsApi } from '../../../test/test-runs-api';
import { TestPage } from './test-page';

const server = startMswServer();

function renderPage(onChange = vi.fn<() => void>()) {
  const api = fakeTestRunsApi();
  server.use(...api.handlers);
  renderRoutes(`/agents/${AGENT_ID}/test`, [
    {
      path: '/agents/$agentId/test',
      component: () => <TestPage agentId={AGENT_ID} onChange={onChange} />,
    },
    { path: '/agents', component: () => <p>Agents</p> },
  ]);
  return { api, onChange };
}

const list = () => screen.getByRole('list', { name: 'Scenarios' });
const status = () => screen.getByRole('status', { name: 'Test run status' });
const row = (name: string) =>
  within(list())
    .getAllByRole('listitem')
    .find((item) => item.textContent.includes(name));

describe('Test page', () => {
  it('lists the scenarios before any run', async () => {
    renderPage();
    expect(await screen.findByRole('heading', { name: 'Completes the survey' })).toBeVisible();
    expect(row('Completes the survey')).toHaveTextContent(/Required.*Not run/);
    expect(row('Asks to be called back')).not.toHaveTextContent('Required');
    await waitFor(() => {
      expect(status()).toHaveTextContent('No test run yet.');
    });
    expect(screen.getByText(/defaults for this purpose/)).toBeVisible();
    expect(screen.getByRole('button', { name: 'Run tests' })).toHaveAttribute(
      'data-variant',
      'primary',
    );
    expect(screen.getByRole('heading', { name: 'Completes the survey' })).toBeVisible();
    expect(screen.getByText('Run the tests to see this conversation.')).toBeVisible();
    expect(
      within(screen.getByRole('list', { name: 'Rubric checks' })).getByText('Not scored'),
    ).toBeVisible();
  });

  it('runs the tests and shows the verdict and each result', async () => {
    const { api, onChange } = renderPage();
    api.state.failing = ['asks-to-call-back'];
    fireEvent.click(await screen.findByRole('button', { name: 'Run tests' }));

    await waitFor(() => {
      expect(status()).toHaveTextContent('Passed: 1 of 1 required scenario passed.');
    });
    expect(api.state.started).toBe(1);
    expect(screen.getByText('fake-llm · fake-decisions · seed 1')).toBeVisible();
    expect(row('Completes the survey')).toHaveTextContent('Passed');
    expect(row('Asks to be called back')).toHaveTextContent('Failed');
    await waitFor(() => {
      expect(onChange).toHaveBeenCalled();
    });

    const transcript = screen.getByRole('region', { name: 'Transcript' });
    expect(within(transcript).getAllByRole('listitem')[1]).toHaveTextContent(
      'Caller:Hello from Completes the survey.',
    );
    expect(screen.getByText(/The caller hung up/)).toBeVisible();
    expect(screen.getByText(/3\.2 s/)).toBeVisible();

    fireEvent.click(within(list()).getByRole('button', { name: /^Asks to be called back/ }));
    const checks = screen.getByRole('list', { name: 'Rubric checks' });
    expect(within(checks).getByText('Failed')).toBeVisible();
    expect(within(checks).getByText('Expected yes · answered no (0.10)')).toBeVisible();
  });

  it('shows a run in progress and cancels it', async () => {
    const { api } = renderPage();
    api.state.holdRuns = true;
    fireEvent.click(await screen.findByRole('button', { name: 'Run tests' }));
    expect(await screen.findByRole('progressbar', { name: 'Test run progress' })).toBeVisible();
    await waitFor(() => {
      expect(status()).toHaveTextContent('Running: 0 of 2 scenarios done.');
    });
    expect(row('Completes the survey')).toHaveTextContent('Running');

    fireEvent.click(screen.getByRole('button', { name: 'Cancel run' }));
    await waitFor(() => {
      expect(status()).toHaveTextContent('Cancelled.');
    });
    expect(row('Completes the survey')).toHaveTextContent('Skipped');
    expect(screen.getByRole('button', { name: 'Run tests' })).toBeVisible();
  });

  it('says when the latest run tested an earlier draft', async () => {
    const { api } = renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Run tests' }));
    await waitFor(() => {
      expect(status()).toHaveTextContent(/Passed/);
    });
    api.state.current = false;
    // Saving the set changes the draft, so the list is read again.
    fireEvent.click(within(list()).getByRole('button', { name: /^Asks to be called back/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Save scenario' }));
    expect(await screen.findByText(/tested an earlier version of the agent/)).toBeVisible();
  });

  it('says why a run could not start', async () => {
    renderPage();
    server.use(
      http.post('*/api/v1/test-runs', () =>
        HttpResponse.json(
          {
            type: 'about:blank',
            title: 'Service Unavailable',
            status: 503,
            detail: "Test runs aren't set up on this server: it has no models for them.",
          },
          { status: 503, headers: { 'content-type': 'application/problem+json' } },
        ),
      ),
    );
    fireEvent.click(await screen.findByRole('button', { name: 'Run tests' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      "Test runs aren't set up on this server",
    );
  });

  it('adds, edits and deletes scenarios, and keeps one required', async () => {
    const { api, onChange } = renderPage();
    fireEvent.click(await screen.findByRole('button', { name: 'Add scenario' }));
    const dialog = screen.getByRole('dialog', { name: 'Add scenario' });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add scenario' }));
    expect(within(dialog).getByText('Write the name.')).toBeVisible();
    expect(within(dialog).getByText('Write the question.')).toBeVisible();

    fireEvent.change(within(dialog).getByLabelText('Name'), { target: { value: 'Books later' } });
    fireEvent.change(within(dialog).getByLabelText("Caller's goal"), {
      target: { value: 'Ask to book next week.' },
    });
    fireEvent.change(within(dialog).getByLabelText('Rubric question'), {
      target: { value: 'Did the agent offer a time?' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add check' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove check 2' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add scenario' }));
    expect(await within(list()).findByText('Books later')).toBeVisible();
    expect(api.state.savedBodies).toHaveLength(1);
    expect(api.state.scenarios.at(-1)).toMatchObject({
      id: 'books-later',
      name: 'Books later',
      required: false,
      maxTurns: 8,
      rubric: [{ question: 'Did the agent offer a time?', expected: 'yes' }],
    });
    await waitFor(() => {
      expect(onChange).toHaveBeenCalled();
    });

    // Deleting the only required scenario is refused before anything is sent.
    fireEvent.click(within(list()).getByRole('button', { name: /^Completes the survey/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Delete' }));
    fireEvent.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Delete' }),
    );
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'Keep at least one scenario required',
    );
    expect(api.state.savedBodies).toHaveLength(1);

    fireEvent.click(within(list()).getByRole('button', { name: /^Books later/ }));
    await screen.findByRole('heading', { name: 'Books later' });
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    fireEvent.click(
      within(screen.getByRole('alertdialog')).getByRole('button', { name: 'Delete' }),
    );
    await waitFor(() => {
      expect(within(list()).queryByText('Books later')).toBeNull();
    });
    expect(api.state.savedBodies).toHaveLength(2);
  });

  it('says when saving the scenarios failed', async () => {
    renderPage();
    server.use(http.put('*/api/v1/agents/:id/scenarios', () => problemResponse(503)));
    fireEvent.click(await screen.findByRole('button', { name: /^Asks to be called back/ }));
    fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Save scenario' }));
    expect(await screen.findByRole('alert')).toHaveTextContent("Couldn't save the scenarios.");
  });

  it('says when the agent is gone, and offers a retry when loading fails', async () => {
    renderPage();
    server.use(http.get('*/api/v1/agents/:id/scenarios', () => problemResponse(404)));
    expect(await screen.findByText("This agent doesn't exist")).toBeVisible();
    expect(screen.getByRole('link', { name: 'Go to Agents' })).toBeVisible();
  });

  it('offers a retry when the scenarios fail to load', async () => {
    renderPage();
    server.use(http.get('*/api/v1/agents/:id/scenarios', () => problemResponse(503)));
    expect(await screen.findByRole('button', { name: 'Try again' })).toBeVisible();
  });
});
