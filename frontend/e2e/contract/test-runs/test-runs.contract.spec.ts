// Contract: test runs v1 (end to end). Pinned by the tag contract/test-runs/v1. Never edit this
// file; to change the contract, add a v2.
//
// The Test step of an agent, `/agents/{id}/test`, against the real Rust server and Postgres. The
// server's two models for test runs (the LLM and the decision model) are the deterministic fakes
// in `support/fake-models.ts`, which the backend server starts; nothing else is faked.
// - The stepper's Test step is a link to the page, and the page is a workspace that fits the
//   screen (1280×800 and 1440×900): the agent's scenarios on one side (each with its name, a
//   "Required" tag when required, and the result of the latest run), "Run tests" as the one
//   primary action, and the selected scenario's result on the other side.
// - "Run tests" starts a run at once: its progress shows while it runs ("Test run progress"),
//   then the verdict, each scenario's status, and per scenario its transcript (the agent's
//   locked disclosure first) and its rubric checks, each passed or failed with the answer it
//   expected. A run passes only if every required scenario passes: a failed optional scenario
//   leaves it passed.
// - A passing run on the current draft (with a valid plan) makes setup progress 75 % and ticks
//   the Test step; the result survives a reload.
// - After an edit to the agent, the page says the run tested an earlier version, and progress
//   drops to 50 %.
// - Scenarios are edited in place: "Add scenario" adds one that is saved and survives a reload.
// - auditScreen passes on every state.
import { expect, test, type APIRequestContext, type Locator, type Page } from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const VIEWPORTS = [
  { width: 1280, height: 800 },
  { width: 1440, height: 900 },
] as const;

const RUN_BUDGET_MS = 30_000;

/** Settled by the fake models: every rubric question is answered yes (0.9). */
const SCENARIOS = [
  {
    id: 'takes-the-survey',
    name: 'Takes the survey',
    goal: 'Answer the survey question, then hang up.',
    persona: 'A regular customer.',
    required: true,
    maxTurns: 6,
    rubric: [{ question: 'Did the agent ask how the visit was?', expected: 'yes' }],
  },
  {
    id: 'opts-out',
    name: 'Opts out',
    goal: 'Ask the agent to stop calling you.',
    persona: 'Annoyed.',
    required: true,
    maxTurns: 6,
    rubric: [{ question: "Did the agent confirm you won't be contacted again?", expected: 'yes' }],
  },
  {
    id: 'tricky-check',
    name: 'Tricky check',
    goal: 'Answer briefly, then hang up.',
    persona: 'Busy.',
    required: false,
    maxTurns: 6,
    rubric: [{ question: 'Did the agent claim to be a person?', expected: 'no' }],
  },
];

function fail(what: string, value: unknown): never {
  throw new Error(`unexpected ${what}: ${JSON.stringify(value)}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

async function createAgent(request: APIRequestContext, name: string): Promise<string> {
  const response = await request.post('/api/v1/agents', {
    data: { name, useCase: 'survey', voice: 'af_heart' },
  });
  expect(response.status()).toBe(201);
  const body: unknown = await response.json();
  const id = isRecord(body) ? body['id'] : undefined;
  return typeof id === 'string' ? id : fail('agent', body);
}

/** Saves the use case's template, so the Plan step is done. */
async function saveTemplatePlan(request: APIRequestContext, agentId: string): Promise<void> {
  const plan: unknown = await (await request.get(`/api/v1/agents/${agentId}/plan`)).json();
  if (!isRecord(plan)) fail('plan', plan);
  const saved = await request.put(`/api/v1/agents/${agentId}/plan`, {
    data: { nodes: plan['nodes'], edges: plan['edges'] },
  });
  expect(saved.status()).toBe(200);
}

async function saveScenarios(request: APIRequestContext, agentId: string): Promise<void> {
  const saved = await request.put(`/api/v1/agents/${agentId}/scenarios`, {
    data: { scenarios: SCENARIOS },
  });
  expect(saved.status()).toBe(200);
}

function pageOverflow(page: Page): Promise<number> {
  return page.evaluate(() => document.documentElement.scrollHeight - window.innerHeight);
}

async function expectNoPageScroll(page: Page): Promise<void> {
  expect(await pageOverflow(page), 'the document must not scroll vertically').toBeLessThanOrEqual(
    0,
  );
}

function scenarioList(page: Page): Locator {
  return page.getByRole('list', { name: 'Scenarios', exact: true });
}

function scenarioItem(page: Page, name: string): Locator {
  return scenarioList(page).getByRole('listitem').filter({ hasText: name });
}

function runStatus(page: Page): Locator {
  return page.getByRole('status', { name: 'Test run status', exact: true });
}

function setupProgress(page: Page): Locator {
  return page.getByRole('progressbar', { name: 'Setup progress' });
}

/** Shows the latest run's result of the scenario named `name`. */
async function select(page: Page, name: string): Promise<void> {
  await scenarioList(page)
    .getByRole('button', { name: new RegExp(`^${name}`) })
    .click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
}

test.describe('test runs v1', { tag: '@contract' }, () => {
  test('the stepper leads to the Test step', async ({ page, request }) => {
    const agentId = await createAgent(request, 'Stepper desk');
    await page.goto(`/agents/${agentId}/provision`);
    const steps = page.getByRole('list', { name: 'Setup steps' });
    await steps.getByRole('link', { name: /Test/ }).click();
    await expect(page).toHaveURL(new RegExp(`/agents/${agentId}/test$`));
    await expect(steps.getByRole('link', { name: /Test/ })).toHaveAttribute('aria-current', 'page');
    // Until scenarios are saved, the survey's default scenarios are there to run.
    await expect(scenarioList(page).getByRole('listitem')).not.toHaveCount(0);
    await expect(runStatus(page)).toHaveText(/No test run yet/);
    await auditScreen(page);
  });

  for (const viewport of VIEWPORTS) {
    test(`runs the scenarios and shows each result at ${viewport.width}×${viewport.height}`, async ({
      page,
      request,
    }) => {
      await page.setViewportSize(viewport);
      const agentId = await createAgent(request, `Survey desk ${viewport.width}`);
      await saveTemplatePlan(request, agentId);
      await saveScenarios(request, agentId);
      await page.goto(`/agents/${agentId}/test`);

      await expect(scenarioList(page).getByRole('listitem')).toHaveCount(3);
      await expect(scenarioItem(page, 'Takes the survey')).toContainText('Required');
      await expect(scenarioItem(page, 'Opts out')).toContainText('Required');
      await expect(scenarioItem(page, 'Tricky check')).not.toContainText('Required');
      const run = page.getByRole('button', { name: 'Run tests', exact: true });
      await expect(run).toHaveAttribute('data-variant', 'primary');
      await expect(run).toBeInViewport({ ratio: 1 });
      await expect(setupProgress(page)).toHaveAttribute('aria-valuenow', '50');
      await expectNoPageScroll(page);
      await auditScreen(page);

      // The run starts at once and shows its progress.
      await run.click();
      await expect(page.getByRole('progressbar', { name: 'Test run progress' })).toBeVisible();
      await expect(runStatus(page)).toHaveText(/Queued|Running/);
      await expect(page.getByRole('button', { name: 'Cancel run', exact: true })).toBeVisible();
      await auditScreen(page);

      // Then the verdict: the optional failure doesn't fail the run.
      await expect(runStatus(page)).toHaveText(/Passed/, { timeout: RUN_BUDGET_MS });
      await expect(page.getByRole('progressbar', { name: 'Test run progress' })).toBeHidden();
      await expect(scenarioItem(page, 'Takes the survey')).toContainText('Passed');
      await expect(scenarioItem(page, 'Opts out')).toContainText('Passed');
      await expect(scenarioItem(page, 'Tricky check')).toContainText('Failed');
      await expect(run).toBeVisible();

      // Each scenario's transcript and rubric checks.
      await select(page, 'Takes the survey');
      const transcript = page.getByRole('region', { name: 'Transcript', exact: true });
      const lines = transcript.getByRole('listitem');
      await expect(lines.first()).toContainText(
        `Hi, this is Survey desk ${viewport.width}, an automated AI assistant.`,
      );
      await expect(transcript).toContainText('About an eight, thanks.');
      const checks = page.getByRole('list', { name: 'Rubric checks', exact: true });
      await expect(checks.getByRole('listitem')).toHaveCount(1);
      await expect(checks.getByRole('listitem').first()).toContainText(
        'Did the agent ask how the visit was?',
      );
      await expect(checks.getByRole('listitem').first()).toContainText(/Passed/);
      await expect(checks.getByRole('listitem').first()).toContainText(/Expected yes/);

      await select(page, 'Opts out');
      await expect(transcript).toContainText('Please stop calling me.');
      await expect(page.getByText(/opted out/).first()).toBeVisible();

      await select(page, 'Tricky check');
      await expect(checks.getByRole('listitem').first()).toContainText(/Failed/);
      await expect(checks.getByRole('listitem').first()).toContainText(/Expected no/);

      // The Test step is done.
      await expect(setupProgress(page)).toHaveAttribute('aria-valuenow', '75');
      await expectNoPageScroll(page);
      await auditScreen(page);

      // It survives a reload.
      await page.reload();
      await expect(runStatus(page)).toHaveText(/Passed/);
      await expect(scenarioItem(page, 'Tricky check')).toContainText('Failed');
      await expect(setupProgress(page)).toHaveAttribute('aria-valuenow', '75');

      // An edit makes the run stale.
      const renamed = await request.patch(`/api/v1/agents/${agentId}`, {
        data: { name: `Survey desk ${viewport.width} (late)` },
      });
      expect(renamed.status()).toBe(200);
      await page.reload();
      await expect(page.getByText(/tested an earlier version of the agent/)).toBeVisible();
      await expect(setupProgress(page)).toHaveAttribute('aria-valuenow', '50');
      await expectNoPageScroll(page);
      await auditScreen(page);
    });
  }

  test('adds a scenario in place', async ({ page, request }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const agentId = await createAgent(request, 'Editor desk');
    await saveScenarios(request, agentId);
    await page.goto(`/agents/${agentId}/test`);
    await expect(scenarioList(page).getByRole('listitem')).toHaveCount(3);

    await page.getByRole('button', { name: 'Add scenario', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Add scenario' });
    await dialog.getByLabel('Name', { exact: true }).fill('Asks for a callback');
    await dialog
      .getByLabel("Caller's goal", { exact: true })
      .fill('Ask to be called back tomorrow.');
    await dialog
      .getByLabel('Rubric question', { exact: true })
      .fill('Did the agent offer to call back?');
    await dialog.getByRole('button', { name: 'Add scenario', exact: true }).click();
    await expect(dialog).toBeHidden();

    await expect(scenarioList(page).getByRole('listitem')).toHaveCount(4);
    await expect(scenarioItem(page, 'Asks for a callback')).toBeVisible();
    await expectNoPageScroll(page);
    await auditScreen(page);
    await page.reload();
    await expect(scenarioItem(page, 'Asks for a callback')).toBeVisible();
  });
});
