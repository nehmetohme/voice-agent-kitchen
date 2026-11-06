import { useParams, useSearch } from '@tanstack/react-router';
// The Purposes list and a purpose's page through the user's eyes: roles and names, against the
// in-memory API fake. The sections themselves are in sections.test.tsx.
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { http } from 'msw';
import { describe, expect, it } from 'vitest';

import { problemResponse, SYSTEM_PURPOSE_IDS } from '../../../test/agents-api';
import { startMswServer } from '../../../test/msw-server';
import {
  CUSTOM_ID,
  fakePurposesApi,
  purposeBody,
  type PurposeBody,
} from '../../../test/purposes-api';
import { renderRoutes } from '../../../test/render-routes';
import { PurposePage } from './purpose-page';
import { PurposesOverview } from './purposes-overview';

const server = startMswServer();

function FlowStub({ purposeId, readOnly }: { purposeId: string; readOnly: boolean }) {
  return <p>{`Flow editor for ${purposeId}${readOnly ? ', read-only' : ''}`}</p>;
}

function PurposeAt() {
  const { purposeId = '' } = useParams({ strict: false });
  const { section = 'overview' } = useSearch({ strict: false });
  return <PurposePage purposeId={purposeId} section={section} flow={FlowStub} />;
}

const ROUTES = [
  { path: '/purposes', component: PurposesOverview },
  { path: '/purposes/$purposeId', component: PurposeAt },
];

function use(purposes: PurposeBody[] = [purposeBody()]) {
  const api = fakePurposesApi(purposes);
  server.use(...api.handlers);
  return api;
}

describe('Purposes list', () => {
  it('lists built-in purposes, then the workspace own, with their version and use', async () => {
    use([purposeBody({ latestVersion: 2, usedBy: 1, unsavedChanges: true })]);
    renderRoutes('/purposes', ROUTES);

    const list = await screen.findByRole('list', { name: 'Purposes' });
    const rows = within(list).getAllByRole('listitem');
    expect(rows.map((row) => within(row).getByRole('link').textContent)).toEqual([
      'Customer support',
      'Appointment booking',
      'Clinic bookings',
    ]);
    expect(rows[0]).toHaveTextContent('Built-in');
    expect(rows[0]).toHaveTextContent('Version 1');
    expect(rows[0]).toHaveTextContent('Used by 0 agents');
    expect(rows[2]).toHaveTextContent('Appointment booking');
    expect(rows[2]).toHaveTextContent('Version 2, with changes');
    expect(rows[2]).toHaveTextContent('Used by 1 agent');
    expect(screen.queryByText(/Built-in purposes are read-only/)).not.toBeInTheDocument();
  });

  it('offers one primary action, New purpose, which copies a purpose and opens it', async () => {
    const api = use([]);
    renderRoutes('/purposes', ROUTES);

    const create = await screen.findByRole('button', { name: 'New purpose' });
    expect(create).toHaveAttribute('data-variant', 'primary');
    expect(screen.getByText(/Built-in purposes are read-only/)).toBeVisible();
    fireEvent.click(create);
    const dialog = await screen.findByRole('dialog', { name: 'New purpose' });
    const name = within(dialog).getByRole('textbox', { name: 'Name' });
    expect(name).toHaveValue('Copy of Customer support');
    fireEvent.change(within(dialog).getByRole('combobox', { name: 'Start from' }), {
      target: { value: SYSTEM_PURPOSE_IDS.booking },
    });
    expect(name).toHaveValue('Copy of Appointment booking');
    fireEvent.change(name, { target: { value: ' ' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create purpose' }));
    expect(await within(dialog).findByText('Give the purpose a name.')).toBeVisible();
    fireEvent.change(name, { target: { value: 'Dental bookings' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create purpose' }));

    expect(await screen.findByRole('heading', { name: 'Dental bookings' })).toBeVisible();
    expect(api.purposes.get(CUSTOM_ID)?.draft.complianceClass).toBe('booking');
  });

  it('says when a copy fails', async () => {
    use([]);
    server.use(http.post('*/api/v1/purposes', () => problemResponse(503)));
    renderRoutes('/purposes', ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'New purpose' }));
    const dialog = await screen.findByRole('dialog', { name: 'New purpose' });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create purpose' }));

    expect(await within(dialog).findByText('The request failed.')).toBeVisible();
  });

  it('offers a retry when the list fails', async () => {
    use();
    server.use(http.get('*/api/v1/purposes', () => problemResponse(503)));
    renderRoutes('/purposes', ROUTES);

    expect(await screen.findByRole('alert')).toBeVisible();
    use();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('list', { name: 'Purposes' })).toBeVisible();
  });
});

describe('Purpose page', () => {
  it('shows a new purpose unsaved, autosaves an edit and saves version 1', async () => {
    const api = use();
    renderRoutes(`/purposes/${CUSTOM_ID}`, ROUTES);

    expect(await screen.findByRole('heading', { name: 'Clinic bookings' })).toBeVisible();
    expect(screen.getByText('Not saved as a version yet')).toBeVisible();
    expect(screen.getByText('Appointment booking rules')).toBeVisible();
    const nav = screen.getByRole('navigation', { name: 'Purpose sections' });
    expect(within(nav).getByRole('link', { name: 'Overview' })).toHaveAttribute(
      'aria-current',
      'page',
    );
    expect(screen.queryByRole('link', { name: 'Create agent' })).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('textbox', { name: 'Name' }), {
      target: { value: 'Dental bookings' },
    });
    expect(screen.getByRole('heading', { name: 'Dental bookings' })).toBeVisible();

    await waitFor(
      () => {
        expect(api.patches).toEqual([
          expect.objectContaining({ name: 'Dental bookings', complianceClass: 'booking' }),
        ]);
      },
      { timeout: 2000 },
    );
    const save = screen.getByRole('button', { name: 'Save version 1' });
    expect(save).toHaveAttribute('data-variant', 'primary');
    await waitFor(() => {
      expect(save).toBeEnabled();
    });
    fireEvent.click(save);

    expect(await screen.findByText('Version 1')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Save version 2' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Save version 2' })).toHaveAccessibleDescription(
      'Nothing changed since version 1.',
    );
    expect(screen.getByRole('link', { name: 'Create agent' })).toHaveAttribute(
      'href',
      `/agents/new?purpose=${CUSTOM_ID}`,
    );
  });

  it('edits every overview field and shows the server reason after a refused save', async () => {
    use();
    server.use(
      http.patch('*/api/v1/purposes/:id', () =>
        problemResponse(422, [{ field: 'persona', message: 'Keep it shorter.' }]),
      ),
    );
    renderRoutes(`/purposes/${CUSTOM_ID}`, ROUTES);

    const persona = await screen.findByRole('textbox', { name: 'Persona' });
    fireEvent.change(screen.getByRole('textbox', { name: 'Description' }), {
      target: { value: 'Dental clinics.' },
    });
    fireEvent.change(screen.getByRole('combobox', { name: 'Compliance rules' }), {
      target: { value: 'service' },
    });
    fireEvent.change(persona, { target: { value: 'Be brief.' } });

    expect(await screen.findByText('Keep it shorter.', {}, { timeout: 2000 })).toBeVisible();
    expect(screen.getByText('Customer service rules')).toBeVisible();
    expect(screen.getByRole('button', { name: 'Save version 1' })).toHaveAccessibleDescription(
      'Your last change is still being saved.',
    );
  });

  it('shows a built-in purpose read-only, with Duplicate as its primary action', async () => {
    use([]);
    renderRoutes(`/purposes/${SYSTEM_PURPOSE_IDS.booking}?section=flow`, ROUTES);

    expect(await screen.findByRole('heading', { name: 'Appointment booking' })).toBeVisible();
    expect(screen.getByText('Built-in')).toBeVisible();
    expect(screen.getByText('Version 1')).toBeVisible();
    expect(screen.getByText(/Duplicate this one to change it/)).toBeVisible();
    expect(
      screen.getByText(`Flow editor for ${SYSTEM_PURPOSE_IDS.booking}, read-only`),
    ).toBeVisible();
    expect(screen.queryByRole('button', { name: /Save version/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('status', { name: 'Save status' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Duplicate' })).toHaveAttribute(
      'data-variant',
      'primary',
    );
    expect(screen.getByRole('link', { name: 'Create agent' })).toHaveAttribute(
      'href',
      '/agents/new?purpose=booking',
    );
    expect(screen.getByRole('region', { name: 'Flow' })).toHaveAttribute('tabindex', '0');
  });

  it('duplicates a built-in purpose and opens the copy', async () => {
    use([]);
    renderRoutes(`/purposes/${SYSTEM_PURPOSE_IDS.booking}`, ROUTES);

    fireEvent.click(await screen.findByRole('button', { name: 'Duplicate' }));
    const dialog = await screen.findByRole('dialog', { name: 'Duplicate purpose' });
    expect(within(dialog).queryByRole('combobox', { name: 'Start from' })).not.toBeInTheDocument();
    expect(within(dialog).getByRole('textbox', { name: 'Name' })).toHaveValue(
      'Copy of Appointment booking',
    );
    fireEvent.click(within(dialog).getByRole('button', { name: 'Duplicate' }));

    expect(
      await screen.findByRole('heading', { name: 'Copy of Appointment booking' }),
    ).toBeVisible();
    expect(screen.getByRole('button', { name: 'Save version 1' })).toBeEnabled();
  });

  it('waits to save a version while the draft has problems', async () => {
    use([
      purposeBody({
        problems: [
          { code: 'host_not_allowed', message: 'Not allowed.', field: 'tools.http[0].url' },
          { code: 'missing_exit', message: 'Add a path out.', field: 'flow' },
        ],
      }),
    ]);
    renderRoutes(`/purposes/${CUSTOM_ID}?section=compliance`, ROUTES);

    const save = await screen.findByRole('button', { name: 'Save version 1' });
    expect(save).toBeDisabled();
    expect(save).toHaveAccessibleDescription('Fix 2 problems first.');
    expect(screen.getByRole('region', { name: 'Compliance' })).toBeVisible();
  });

  it('says when a purpose does not exist, and offers a retry when it fails to load', async () => {
    use([]);
    renderRoutes('/purposes/019a5a5a-0000-7000-8000-0000000000ff', ROUTES);

    expect(
      await screen.findByRole('heading', { name: "This purpose doesn't exist" }),
    ).toBeVisible();
    expect(screen.getByRole('link', { name: 'Go to Purposes' })).toHaveAttribute(
      'href',
      '/purposes',
    );
  });

  it('offers a retry when the purpose fails to load', async () => {
    use();
    server.use(http.get('*/api/v1/purposes/:id', () => problemResponse(503)));
    renderRoutes(`/purposes/${CUSTOM_ID}`, ROUTES);

    expect(await screen.findByRole('alert')).toBeVisible();
    use();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('heading', { name: 'Clinic bookings' })).toBeVisible();
  });
});
