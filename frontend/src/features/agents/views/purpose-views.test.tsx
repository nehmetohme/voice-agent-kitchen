// An agent's purpose through the user's eyes: the workspace's purposes in the pickers, a
// searchable one past seven (HICK-3), and "Update available" with what an update changes.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { http } from 'msw';
import { describe, expect, it } from 'vitest';

import { fakeAgentPurposeApi, updateBody } from '../../../test/agent-purpose-api';
import { AGENT_ID, agentBody, fakeAgentsApi, problemResponse } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import {
  CUSTOM_ID,
  fakePurposesApi,
  purposeBody,
  systemPurposeBody,
  type PurposeBody,
} from '../../../test/purposes-api';
import { renderRoutes } from '../../../test/render-routes';
import { CreateAgentPage } from './create-agent-page';
import { ProvisionPage } from './provision-page';

const server = startMswServer();

const PINNED = agentBody({
  useCase: 'booking',
  purpose: { id: CUSTOM_ID, version: 1, system: false },
});

function ProvisionAtId() {
  return <ProvisionPage agentId={AGENT_ID} />;
}

function CreateAgent() {
  return <CreateAgentPage />;
}

function CreateFromClinic() {
  return <CreateAgentPage initialPurpose={CUSTOM_ID} />;
}

const ROUTES = [
  { path: '/agents/new', component: CreateAgent },
  { path: '/agents/from-clinic', component: CreateFromClinic },
  { path: '/agents/$agentId/provision', component: ProvisionAtId },
];

const CLINIC = purposeBody({ latestVersion: 2, unsavedChanges: false });

function use({
  update = null,
  purposes = [CLINIC],
}: { update?: Record<string, unknown> | null; purposes?: PurposeBody[] } = {}) {
  const agents = fakeAgentsApi([PINNED]);
  const purpose = fakeAgentPurposeApi(update);
  server.use(...agents.handlers, ...fakePurposesApi(purposes).handlers, ...purpose.handlers);
  return { agents, purpose };
}

describe('Update available', () => {
  it('says what an update changes and keeps an edited plan unless told otherwise', async () => {
    const { purpose } = use({ update: updateBody() });
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'Update available' }));
    const dialog = await screen.findByRole('dialog', { name: 'Update to version 2' });
    expect(dialog).toHaveTextContent(
      'Clinic bookings has a new version. Your agent uses version 1 until you update.',
    );
    const changes = within(dialog).getByRole('list', { name: 'What changes' });
    expect(
      within(changes)
        .getAllByRole('listitem')
        .map((item) => item.textContent),
    ).toEqual(['Tools: Adds book_slot.', 'Starting flow: The starting flow changes.']);
    expect(dialog).toHaveTextContent('Your plan has your own edits, so it stays as it is');
    const keep = within(dialog).getByRole('radio', { name: 'Keep my plan' });
    expect(keep).toBeChecked();
    fireEvent.click(
      within(dialog).getByRole('radio', { name: 'Replace my plan with the new starting flow' }),
    );
    fireEvent.click(within(dialog).getByRole('button', { name: 'Update to version 2' }));

    await waitFor(() => {
      expect(purpose.requests).toEqual([{ version: 2, replacePlan: true }]);
    });
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Update available' })).not.toBeInTheDocument();
    });
  });

  it('never replaces a plan without edits on request, and says the plan follows', async () => {
    const { purpose } = use({ update: updateBody({ plan: 'follows', planEdited: false }) });
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'Update available' }));
    const dialog = await screen.findByRole('dialog', { name: 'Update to version 2' });
    expect(dialog).toHaveTextContent(
      'No plan is saved yet, so the agent shows the new starting flow.',
    );
    expect(within(dialog).queryByRole('radio')).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Update to version 2' }));

    await waitFor(() => {
      expect(purpose.requests).toEqual([{ version: 2, replacePlan: false }]);
    });
  });

  it('shows nothing when the agent pins the latest version', async () => {
    use();
    renderRoutes(`/agents/${AGENT_ID}/provision`, ROUTES);

    expect(await screen.findByRole('heading', { level: 2, name: 'Front desk' })).toBeVisible();
    expect(screen.getByRole('combobox', { name: 'Purpose' })).toHaveValue(CUSTOM_ID);
    expect(screen.queryByRole('button', { name: 'Update available' })).not.toBeInTheDocument();
  });
});

describe('Purpose pickers', () => {
  it('offers the workspace purposes with a saved version and sends a purpose by ID', async () => {
    const draft = purposeBody({ id: '019a5a5a-0000-7000-8000-000000000002', latestVersion: null });
    const { agents } = use({ purposes: [CLINIC, draft] });
    let created: unknown;
    server.use(
      http.post('*/api/v1/agents', async ({ request }) => {
        created = await request.json();
        return problemResponse(503);
      }),
    );
    renderRoutes('/agents/from-clinic', ROUTES);

    const purpose = await screen.findByRole('combobox', { name: 'Purpose' });
    await waitFor(() => {
      expect(
        within(purpose)
          .getAllByRole('option')
          .map((option) => option.textContent),
      ).toEqual(['Choose a purpose', 'Customer support', 'Appointment booking', 'Clinic bookings']);
    });
    expect(purpose).toHaveValue(CUSTOM_ID);
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Clinic line' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Create agent' }));

    await waitFor(() => {
      expect(created).toMatchObject({
        name: 'Clinic line',
        useCase: 'booking',
        purposeId: CUSTOM_ID,
      });
    });
    expect(agents.agents.size).toBe(1);
  });

  it('turns the purpose picker into a search past seven purposes', async () => {
    const extra = ['service', 'survey', 'telemarketing', 'custom'] as const;
    const second = purposeBody({
      id: '019a5a5a-0000-7000-8000-000000000002',
      latestVersion: 1,
      draft: { ...CLINIC.draft, name: 'Dental reminders' },
    });
    use({ purposes: [...extra.map((useCase) => systemPurposeBody(useCase)), CLINIC, second] });
    renderRoutes('/agents/new', ROUTES);

    await waitFor(() => {
      expect(screen.getByRole('combobox', { name: 'Purpose' }).tagName).toBe('INPUT');
    });
    const purpose = screen.getByRole('combobox', { name: 'Purpose' });
    fireEvent.keyDown(purpose, { key: 'ArrowDown' });
    fireEvent.change(purpose, { target: { value: 'dental' } });
    const list = await screen.findByRole('listbox');
    fireEvent.click(within(list).getByRole('option', { name: 'Dental reminders' }));

    await waitFor(() => {
      expect(purpose).toHaveValue('Dental reminders');
    });
  });
});
