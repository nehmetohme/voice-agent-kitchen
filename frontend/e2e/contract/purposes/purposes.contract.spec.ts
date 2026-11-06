// Contract: purposes v1 (end to end). Pinned by the tag contract/purposes/v1. Never edit this
// file; to change the contract, add a v2.
//
// Runs against the real Rust server and Postgres that playwright.config.ts starts; never
// against mocks. The path the owner asked for:
// - "Purposes" sits in the primary navigation between Agents and Settings; the list shows the
//   six built-in purposes, each a link to its page.
// - A built-in purpose is read-only, and its one primary action is "Duplicate".
// - The duplicate is a draft, "Not saved as a version yet". On its Tools section a built-in
//   tool is turned on and a custom HTTP tool is added (name, what it does, method, a URL on the
//   allowed host, a secret by name, confirm first, one input); on its Guardrails section an
//   allowed topic and a never-rule are added. Every edit autosaves within 2 s.
// - "Save version 1" makes version 1; "Create agent" opens the create form with the purpose
//   chosen, and the new agent pins version 1, with no update to offer.
// - A newer version ("Save version 2") makes the agent's page offer "Update available"; its
//   dialog says what changes, and updating moves the agent to version 2.
// - The purpose page and the agent page never scroll the document at 1280×800, and
//   auditScreen passes on every screen and dialog.
import { expect, test, type APIRequestContext, type Page } from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const AUTOSAVE_BUDGET_MS = 2_000;
const BOOKING_ID = '019a0000-0000-7000-8000-000000000003';
const BUILT_IN = [
  'Customer support',
  'Customer service',
  'Appointment booking',
  'Survey',
  'Outbound sales',
  'Something else',
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function record(value: unknown, what: string): Record<string, unknown> {
  if (isRecord(value)) return value;
  throw new Error(`unexpected ${what}: ${JSON.stringify(value)}`);
}

async function getJson(request: APIRequestContext, path: string): Promise<Record<string, unknown>> {
  const response = await request.get(path);
  expect(response.status(), path).toBe(200);
  return record(await response.json(), path);
}

/** A host HTTP tools may call: the first allowed one (a wildcard gets a subdomain). */
async function allowedHost(request: APIRequestContext): Promise<string> {
  const catalog = await getJson(request, '/api/v1/catalog/tools');
  const hosts = catalog['allowedHosts'];
  if (!Array.isArray(hosts) || typeof hosts[0] !== 'string') {
    throw new Error(`the e2e server allows no tool host: ${JSON.stringify(catalog)}`);
  }
  return hosts[0].startsWith('*.') ? `api.${hosts[0].slice(2)}` : hosts[0];
}

async function pinnedVersion(request: APIRequestContext, agentId: string): Promise<unknown> {
  return (await getJson(request, `/api/v1/agents/${agentId}/purpose`))['version'];
}

/** How far the document itself scrolls vertically; 0 when the page fits the viewport. */
async function expectNoPageScroll(page: Page): Promise<void> {
  const overflow = await page.evaluate(
    () => document.documentElement.scrollHeight - window.innerHeight,
  );
  expect(overflow, 'the document must not scroll vertically').toBeLessThanOrEqual(0);
}

async function expectSaved(page: Page): Promise<void> {
  await expect(page.getByRole('status', { name: 'Save status' })).toHaveText('Saved', {
    timeout: AUTOSAVE_BUDGET_MS,
  });
}

async function openSection(page: Page, section: string): Promise<void> {
  const link = page
    .getByRole('navigation', { name: 'Purpose sections' })
    .getByRole('link', { name: section, exact: true });
  await link.click();
  await expect(link).toHaveAttribute('aria-current', 'page');
}

test.describe('purposes v1', { tag: '@contract' }, () => {
  test('duplicate a built-in purpose, add a tool and guardrails, version it and update an agent', async ({
    page,
    request,
  }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1280, height: 800 });
    const host = await allowedHost(request);
    const name =
      test.info().retry === 0 ? 'Clinic bookings' : `Clinic bookings ${test.info().retry}`;

    // The list: in the navigation between Agents and Settings, the six built-ins first.
    await page.goto('/agents');
    const nav = page.getByRole('navigation', { name: 'Primary' }).getByRole('link');
    await expect(nav.first()).toHaveAccessibleName('Agents');
    await expect(nav.nth(1)).toHaveAccessibleName('Purposes');
    await expect(nav.last()).toHaveAccessibleName('Settings');
    await nav.nth(1).click();
    await expect(page).toHaveURL(/\/purposes$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Purposes' })).toBeVisible();
    await expect(nav.nth(1)).toHaveAttribute('aria-current', 'page');
    const list = page.getByRole('list', { name: 'Purposes' });
    await Promise.all(
      BUILT_IN.map((label) =>
        expect(list.getByRole('link', { name: label, exact: true })).toBeVisible(),
      ),
    );
    await auditScreen(page);

    // A built-in purpose is read-only; its one primary action is Duplicate.
    await list.getByRole('link', { name: 'Appointment booking', exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/purposes/${BOOKING_ID}`));
    await expect(page.getByRole('heading', { name: 'Appointment booking' })).toBeVisible();
    await expect(page.getByText('Built-in', { exact: true })).toBeVisible();
    const primary = page.locator('[data-variant="primary"]:visible');
    await expect(primary).toHaveCount(1);
    await expect(primary).toHaveAccessibleName('Duplicate');
    await expectNoPageScroll(page);
    await auditScreen(page);

    await primary.click();
    const duplicate = page.getByRole('dialog', { name: 'Duplicate purpose' });
    const copyName = duplicate.getByLabel('Name', { exact: true });
    await expect(copyName).toHaveValue('Copy of Appointment booking');
    await auditScreen(page);
    await copyName.fill(name);
    await duplicate.getByRole('button', { name: 'Duplicate', exact: true }).click();
    await expect(page).not.toHaveURL(new RegExp(BOOKING_ID));
    await expect(page).toHaveURL(/\/purposes\/[0-9a-f-]{36}/);
    const purposeId = /\/purposes\/([0-9a-f-]{36})/.exec(page.url())?.[1] ?? '';
    expect(purposeId).not.toBe(BOOKING_ID);
    await expect(page.getByRole('heading', { name })).toBeVisible();
    await expect(page.getByText('Not saved as a version yet')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Save version 1', exact: true })).toHaveAttribute(
      'data-variant',
      'primary',
    );

    // Tools: a built-in tool on, and a custom HTTP tool on the allowed host.
    await openSection(page, 'Tools');
    const builtIns = page.getByRole('group', { name: 'Built-in tools' });
    await builtIns.getByLabel('Transfer to a person', { exact: true }).check();
    await expect(builtIns.getByLabel('Opt out', { exact: false })).toBeChecked();
    await expect(builtIns.getByLabel('Opt out', { exact: false })).toBeDisabled();
    await expectSaved(page);

    await page.getByRole('button', { name: 'Add HTTP tool', exact: true }).click();
    const toolDialog = page.getByRole('dialog', { name: 'Add HTTP tool' });
    await toolDialog.getByLabel('Tool name', { exact: true }).fill('book_slot');
    await toolDialog
      .getByLabel('What it does', { exact: true })
      .fill("Book a free slot in the clinic's calendar.");
    await toolDialog.getByLabel('Method', { exact: true }).selectOption('POST');
    await toolDialog.getByLabel('URL', { exact: true }).fill(`https://${host}/v1/slots`);
    await toolDialog.getByLabel('Secret name', { exact: true }).fill('CALENDAR_API_KEY');
    await toolDialog.getByLabel('Confirm with the caller first', { exact: true }).check();
    await toolDialog.getByRole('button', { name: 'Add input', exact: true }).click();
    const input = toolDialog.getByRole('group', { name: 'Input 1' });
    await input.getByLabel('Name', { exact: true }).fill('date');
    await input.getByLabel('Description', { exact: true }).fill('The day, as YYYY-MM-DD');
    await input.getByLabel('Required', { exact: true }).check();
    await auditScreen(page);
    await toolDialog.getByRole('button', { name: 'Add tool', exact: true }).click();
    await expect(toolDialog).toBeHidden();
    await expect(page.getByRole('group', { name: 'book_slot' })).toBeVisible();
    await expectSaved(page);
    await expect
      .poll(async () => {
        const purpose = await getJson(request, `/api/v1/purposes/${purposeId}`);
        const tools = record(record(purpose['draft'], 'draft')['tools'], 'tools');
        return { tools, problems: purpose['problems'] };
      })
      .toMatchObject({
        tools: {
          builtins: ['transfer_to_human', 'mark_do_not_call'],
          http: [
            {
              name: 'book_slot',
              method: 'POST',
              url: `https://${host}/v1/slots`,
              secret: 'CALENDAR_API_KEY',
              confirm: true,
              inputs: { properties: { date: { type: 'string' } }, required: ['date'] },
            },
          ],
        },
        problems: [],
      });
    await expectNoPageScroll(page);
    await auditScreen(page);

    // Guardrails: an allowed topic and a never-rule; the pack's claims show, locked.
    await openSection(page, 'Guardrails');
    await page.getByLabel('New topic', { exact: true }).fill('Booking appointments');
    await page.getByRole('button', { name: 'Add topic', exact: true }).click();
    await page.getByLabel('New never-rule', { exact: true }).fill('Give medical advice');
    await page.getByRole('button', { name: 'Add never-rule', exact: true }).click();
    await expect(page.getByRole('list', { name: 'Allowed topics' })).toContainText(
      'Booking appointments',
    );
    await expect(page.getByRole('list', { name: 'Never' })).toContainText('Give medical advice');
    await expect(page.getByRole('list', { name: 'From the rule pack' })).not.toBeEmpty();
    await expectSaved(page);
    await expect
      .poll(async () => {
        const purpose = await getJson(request, `/api/v1/purposes/${purposeId}`);
        return record(record(purpose['draft'], 'draft')['guardrails'], 'guardrails');
      })
      .toMatchObject({ allowedTopics: ['Booking appointments'], never: ['Give medical advice'] });
    await expectNoPageScroll(page);
    await auditScreen(page);

    // Version 1, then an agent made from it.
    await page.getByRole('button', { name: 'Save version 1', exact: true }).click();
    await expect(page.getByText('Version 1', { exact: true })).toBeVisible();
    expect((await getJson(request, `/api/v1/purposes/${purposeId}`))['latestVersion']).toBe(1);
    await page.getByRole('link', { name: 'Create agent', exact: true }).click();
    const form = page.getByRole('form', { name: 'Create agent' });
    await expect(form.getByLabel('Purpose', { exact: true })).toHaveValue(purposeId);
    await form.getByLabel('Name', { exact: true }).fill(`${name} desk`);
    await expect(form.getByLabel('Voice', { exact: true })).toBeEnabled();
    await auditScreen(page);
    await form.getByRole('button', { name: 'Create agent', exact: true }).click();
    await expect(page).toHaveURL(/\/agents\/[0-9a-f-]{36}\/provision$/);
    const agentId = /\/agents\/([0-9a-f-]{36})\//.exec(page.url())?.[1] ?? '';
    await expect(page.getByLabel('Purpose', { exact: true })).toHaveValue(purposeId);
    expect(await pinnedVersion(request, agentId)).toBe(1);
    await expect(page.getByRole('status', { name: 'Save status' })).toHaveText('Saved');
    await expect(page.getByRole('button', { name: 'Update available' })).toHaveCount(0);

    // Version 2: the agent's page offers the update and says what changes.
    await page.goto(`/purposes/${purposeId}`);
    await openSection(page, 'Overview');
    await page
      .getByLabel('Description', { exact: true })
      .fill('Books, moves and cancels clinic appointments.');
    await expectSaved(page);
    await expect(page.getByText('Changes since version 1')).toBeVisible();
    await page.getByRole('button', { name: 'Save version 2', exact: true }).click();
    await expect(page.getByText('Version 2', { exact: true })).toBeVisible();

    await page.goto(`/agents/${agentId}/provision`);
    const update = page.getByRole('button', { name: 'Update available' });
    await expect(update).toBeVisible();
    await expect(update).not.toHaveAttribute('data-variant', 'primary');
    await expectNoPageScroll(page);
    await auditScreen(page);
    await update.click();
    const dialog = page.getByRole('dialog', { name: 'Update to version 2' });
    await expect(dialog).toContainText('Description');
    await auditScreen(page);
    await dialog.getByRole('button', { name: 'Update to version 2', exact: true }).click();
    await expect(dialog).toBeHidden();
    await expect(update).toHaveCount(0);
    expect(await pinnedVersion(request, agentId)).toBe(2);
    await expectNoPageScroll(page);

    // Back on the list: the purpose is used by the agent.
    await page.goto('/purposes');
    const row = page.getByRole('listitem').filter({ has: page.getByRole('link', { name }) });
    await expect(row).toContainText('Version 2');
    await expect(row).toContainText('Used by 1 agent');
    await auditScreen(page);
  });
});
