// An in-memory fake of the agent's purpose (MSW): `GET /agents/:id/purpose` and its update,
// shaped like contracts/openapi.json.
import { HttpResponse, http } from 'msw';

import { CUSTOM_ID, objectBody } from './purposes-api';

/** The agent pins version 1 of the custom purpose; with `update`, version 2 exists. */
function agentPurposeBody(update: Record<string, unknown> | null) {
  return {
    id: CUSTOM_ID,
    version: 1,
    name: 'Clinic bookings',
    system: false,
    latestVersion: update === null ? 1 : 2,
    update,
  };
}

/** A `PurposeUpdate` to version 2 that changes the tools and keeps an edited plan. */
export function updateBody(overrides: Record<string, unknown> = {}) {
  return {
    version: 2,
    changes: [
      { area: 'tools', summary: 'Adds book_slot.' },
      { area: 'flow', summary: 'The starting flow changes.' },
    ],
    plan: 'kept',
    planEdited: true,
    ...overrides,
  };
}

/** Offers `update` until the agent moves; `requests` records every update body. */
export function fakeAgentPurposeApi(update: Record<string, unknown> | null) {
  let current = agentPurposeBody(update);
  const requests: Record<string, unknown>[] = [];
  const handlers = [
    http.get('*/api/v1/agents/:id/purpose', () => HttpResponse.json(current)),
    http.post('*/api/v1/agents/:id/purpose/update', async ({ request }) => {
      requests.push(await objectBody(request));
      current = { ...current, version: current.latestVersion, update: null };
      return HttpResponse.json(current);
    }),
  ];
  return { handlers, requests };
}
