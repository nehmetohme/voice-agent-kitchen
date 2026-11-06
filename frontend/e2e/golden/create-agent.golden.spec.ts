// Golden path `create-agent` (docs/ux-guardrails/golden-paths.md): from the Agents page to a
// saved agent at 25 % setup, within 8 interactions and 60 s, with auditScreen on each screen.
// The contract spec (e2e/contract/agent-provisioning) pins the same path from an empty list;
// this one starts wherever the list is, as people do.
import { expect, test } from '@playwright/test';

import { auditScreen } from '../support/ux-audit';

const BUDGET = { interactions: 8, ms: 60_000 };

test('create an agent from the Agents page', { tag: '@golden' }, async ({ page }) => {
  const started = Date.now();
  let interactions = 0;
  const act = async (step: () => Promise<unknown>) => {
    interactions += 1;
    await step();
  };

  await page.goto('/agents');
  const create = page.getByRole('link', { name: 'Create agent', exact: true });
  await expect(create).toHaveAttribute('data-variant', 'primary');
  await auditScreen(page);

  await act(() => create.click());
  const form = page.getByRole('form', { name: 'Create agent' });
  await expect(form.getByLabel('Voice', { exact: true })).toBeEnabled();
  await auditScreen(page);
  await act(() => form.getByLabel('Name', { exact: true }).fill('Appointment desk'));
  // A native select up to seven purposes; past seven (the workspace's own purposes) a
  // searchable combobox (HICK-3), which a person types into and picks from.
  const purpose = form.getByLabel('Purpose', { exact: true });
  await act(async () => {
    if (await purpose.evaluate((field) => field.tagName === 'SELECT')) {
      await purpose.selectOption('booking');
      return;
    }
    await purpose.fill('Appointment booking');
    await page.getByRole('option', { name: 'Appointment booking', exact: true }).click();
  });
  // The recommended voice is preselected, so the path needs no voice step.
  await act(() => form.getByRole('button', { name: 'Create agent', exact: true }).click());

  // End state: the agent's page, saved, with setup progress at 25 %.
  await expect(page).toHaveURL(/\/agents\/[0-9a-f-]{36}\/provision$/);
  await expect(page.getByRole('status')).toHaveText('Saved');
  await expect(page.getByRole('progressbar', { name: 'Setup progress' })).toHaveAttribute(
    'aria-valuenow',
    '25',
  );
  await expect(page.getByLabel('Name', { exact: true })).toHaveValue('Appointment desk');
  expect(interactions).toBeLessThanOrEqual(BUDGET.interactions);
  expect(Date.now() - started).toBeLessThanOrEqual(BUDGET.ms);
  await auditScreen(page);
});
