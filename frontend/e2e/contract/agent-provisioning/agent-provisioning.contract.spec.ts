// Contract: agent provisioning v1 (end to end). Pinned by the signed tag
// contract/agent-provisioning/v1. Never edit this file; to change the contract, add a v2.
//
// Runs against the real Rust server and a throwaway Postgres database that
// playwright.config.ts starts; never against mocks. The golden path:
// - the empty Agents list offers exactly one primary action, "Create agent";
// - the create form asks for 3 fields, and its purpose list has at most 7 options;
// - creating an agent takes at most 8 interactions and lands on its provision page;
// - setup progress is 25 % and survives a reload;
// - an edit autosaves ("Saved") within 2 s, with no Save button, and survives a reload;
// - auditScreen passes on the list, the create form and the provision page.
import { expect, test, type APIRequestContext } from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const MAX_INTERACTIONS = 8;
const AUTOSAVE_BUDGET_MS = 2_000;
const CREATE_BUDGET_MS = 60_000;

/** The IDs in a `GET /api/v1/agents` body: `{ "items": [{ "id": "…" }, …] }`. */
function agentIds(body: unknown): string[] {
  const items: unknown =
    typeof body === 'object' && body !== null && 'items' in body ? body.items : undefined;
  if (!Array.isArray(items)) throw new Error(`unexpected agent list: ${JSON.stringify(body)}`);
  return items.map((item: unknown) => {
    const id: unknown = typeof item === 'object' && item !== null && 'id' in item ? item.id : null;
    if (typeof id !== 'string') throw new Error(`agent without an id: ${JSON.stringify(item)}`);
    return id;
  });
}

/** Archives every agent, so the list starts empty even when a test is retried. */
async function archiveAllAgents(request: APIRequestContext): Promise<void> {
  const list = await request.get('/api/v1/agents');
  expect(list.status()).toBe(200);
  const ids = agentIds(await list.json());
  const archived = await Promise.all(ids.map((id) => request.delete(`/api/v1/agents/${id}`)));
  expect(archived.map((response) => response.status())).toEqual(ids.map(() => 204));
}

/** Counts what the user does on the way (Parkinson: a short path). */
function interactions() {
  let count = 0;
  return {
    async act(step: () => Promise<unknown>): Promise<void> {
      count += 1;
      await step();
    },
    get count(): number {
      return count;
    },
  };
}

test.describe('agent provisioning v1', { tag: '@contract' }, () => {
  test.beforeEach(async ({ request }) => {
    await archiveAllAgents(request);
  });

  test('create an agent, autosave an edit and keep 25 % setup progress', async ({ page }) => {
    const started = Date.now();
    const user = interactions();

    // The empty list: one primary action.
    await page.goto('/agents');
    await expect(page.getByRole('heading', { name: 'No agents yet' })).toBeVisible();
    const primary = page.locator('[data-variant="primary"]:visible');
    await expect(primary).toHaveCount(1);
    await expect(primary).toHaveAccessibleName('Create agent');
    await auditScreen(page);

    // The create form: three fields, at most seven purposes.
    await user.act(() => primary.click());
    const form = page.getByRole('form', { name: 'Create agent' });
    await expect(form).toBeVisible();
    await expect(form.locator('input:not([type="hidden"]), select, textarea')).toHaveCount(3);
    const purpose = form.getByLabel('Purpose', { exact: true });
    expect(await purpose.locator('option').count()).toBeLessThanOrEqual(7);
    await auditScreen(page);

    await user.act(() => form.getByLabel('Name', { exact: true }).fill('Front desk'));
    await user.act(() => purpose.selectOption('support'));
    await user.act(() => form.getByLabel('Voice', { exact: true }).selectOption('af_heart'));
    await user.act(() => form.getByRole('button', { name: 'Create agent', exact: true }).click());

    // The provision page: saved, 25 % set up, four steps.
    await expect(page).toHaveURL(/\/agents\/[0-9a-f-]{36}\/provision$/);
    expect(user.count).toBeLessThanOrEqual(MAX_INTERACTIONS);
    const progress = page.getByRole('progressbar', { name: 'Setup progress' });
    await expect(progress).toHaveAttribute('aria-valuenow', '25');
    await expect(page.getByRole('list', { name: 'Setup steps' }).getByRole('listitem')).toHaveText([
      /Provision/,
      /Plan/,
      /Test/,
      /Publish/,
    ]);
    const status = page.getByRole('status');
    await expect(status).toHaveText('Saved');
    expect(Date.now() - started).toBeLessThanOrEqual(CREATE_BUDGET_MS);
    await auditScreen(page);

    // Autosave: no Save button; an edit is saved within 2 s and survives a reload.
    await expect(page.getByRole('button', { name: 'Save', exact: true })).toHaveCount(0);
    const name = page.getByLabel('Name', { exact: true });
    await expect(name).toHaveValue('Front desk');
    await name.fill('Front desk (after hours)');
    const editedAt = Date.now();
    await expect(status).not.toHaveText('Saved');
    await expect(status).toHaveText('Saved', { timeout: AUTOSAVE_BUDGET_MS });
    expect(Date.now() - editedAt).toBeLessThanOrEqual(AUTOSAVE_BUDGET_MS);

    await page.reload();
    await expect(page.getByLabel('Name', { exact: true })).toHaveValue('Front desk (after hours)');
    await expect(progress).toHaveAttribute('aria-valuenow', '25');
    await expect(status).toHaveText('Saved');
    await auditScreen(page);

    // Back on the list: the agent is there, and there is still one primary action.
    await page.goto('/agents');
    await expect(page.getByRole('link', { name: 'Front desk (after hours)' })).toBeVisible();
    await expect(page.locator('[data-variant="primary"]:visible')).toHaveCount(1);
    await auditScreen(page);
  });
});
