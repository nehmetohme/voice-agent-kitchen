// Golden path `edit-plan` (docs/ux-guardrails/golden-paths.md): from an agent's page to its
// Plan step, keep the template, change the greeting; each change autosaves within the budget,
// and the end state is a valid plan at 50 % setup, with auditScreen on each screen. The
// contract spec (e2e/contract/plan) pins adding, connecting and deleting steps by keyboard.
import { expect, test } from '@playwright/test';

import { auditScreen } from '../support/ux-audit';

const AUTOSAVE_BUDGET_MS = 2_000;

test('edit the plan in the flow editor', { tag: '@golden' }, async ({ page, request }) => {
  const created = await request.post('/api/v1/agents', {
    data: { name: 'Golden plan', useCase: 'support', voice: 'af_heart' },
  });
  expect(created.status()).toBe(201);
  const body: unknown = await created.json();
  const id = typeof body === 'object' && body !== null && 'id' in body ? body.id : null;
  if (typeof id !== 'string') throw new Error(`no agent id in ${JSON.stringify(body)}`);

  await page.goto(`/agents/${id}/provision`);
  await page.getByRole('link', { name: 'Plan', exact: true }).first().click();
  await expect(page).toHaveURL(new RegExp(`/agents/${id}/plan$`));
  const progress = page.getByRole('progressbar', { name: 'Setup progress' });
  await expect(page.getByRole('group', { name: 'Greeting', exact: true })).toBeVisible();
  await auditScreen(page);

  const status = page.getByRole('status', { name: 'Save status' });
  await page.getByRole('button', { name: 'Use this plan', exact: true }).click();
  await expect(status).toHaveText('Saved', { timeout: AUTOSAVE_BUDGET_MS });
  await expect(progress).toHaveAttribute('aria-valuenow', '50');

  const greeting = page.getByRole('group', { name: 'Greeting', exact: true });
  await greeting.getByRole('button', { name: 'Edit step', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit Greeting' });
  await expect(dialog.getByText('Locked by the rule pack')).toBeVisible();
  await auditScreen(page);
  await dialog.getByLabel('Greeting', { exact: true }).fill('Thanks for calling Golden Dental.');
  await dialog.getByRole('button', { name: 'Save step', exact: true }).click();
  const editedAt = Date.now();
  await expect(status).not.toHaveText('Saved');
  await expect(status).toHaveText('Saved', { timeout: AUTOSAVE_BUDGET_MS });
  expect(Date.now() - editedAt).toBeLessThanOrEqual(AUTOSAVE_BUDGET_MS);

  // End state: valid, problems none, 50 %, and it survives a reload.
  await page.reload();
  await expect(greeting).toContainText('Thanks for calling Golden Dental.');
  await expect(page.getByText('The plan follows every rule.')).toBeVisible();
  await expect(progress).toHaveAttribute('aria-valuenow', '50');
  await auditScreen(page);
});
