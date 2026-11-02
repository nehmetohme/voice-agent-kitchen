// Contract: plan v1 (end to end). Pinned by the signed tag contract/plan/v1. Never edit this
// file; to change the contract, add a v2.
//
// Runs against the real Rust server and a throwaway Postgres database that
// playwright.config.ts starts, never against mocks, after the agent-provisioning contract.
// - Every use case's template is a valid plan; saving it makes setup progress 50 %.
// - The golden path `edit-plan`, by keyboard only: from the agent's page to its Plan step;
//   use the template; add a step, which shows its problems on the step until it is connected;
//   connect it in and out; delete it after a confirmation. Every edit autosaves within 2 s,
//   progress follows the plan (50 % valid, 25 % not), and the result survives a reload.
// - A saved plan's problems show on the step they belong to (a path's on the step it leaves).
// - auditScreen passes on the plan page and on each of its dialogs.
import { expect, test, type APIRequestContext, type Locator, type Page } from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const AUTOSAVE_BUDGET_MS = 2_000;
const USE_CASES = ['support', 'service', 'booking', 'survey', 'telemarketing', 'custom'];

interface Step {
  id: string;
  kind: string;
  label: string;
  text: string;
}

interface Path {
  id: string;
  from: string;
  to: string;
  condition: string;
}

interface Problem {
  code: string;
  message: string;
  nodeId: string | null;
  edgeId: string | null;
}

interface AgentPlan {
  saved: boolean;
  lockedTexts: string[];
  nodes: Step[];
  edges: Path[];
  valid: boolean;
  errors: Problem[];
}

function fail(what: string, value: unknown): never {
  throw new Error(`unexpected ${what}: ${JSON.stringify(value)}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function record(value: unknown, what: string): Record<string, unknown> {
  return isRecord(value) ? value : fail(what, value);
}

function text(value: unknown, what: string): string {
  return typeof value === 'string' ? value : fail(what, value);
}

function list(value: unknown, what: string): unknown[] {
  return Array.isArray(value) ? value : fail(what, value);
}

function optionalText(value: unknown, what: string): string | null {
  return value === undefined || value === null ? null : text(value, what);
}

function readStep(value: unknown): Step {
  const step = record(value, 'step');
  return {
    id: text(step['id'], 'step id'),
    kind: text(step['kind'], 'step kind'),
    label: text(step['label'], 'step label'),
    text: text(step['text'], 'step text'),
  };
}

function readPath(value: unknown): Path {
  const path = record(value, 'path');
  return {
    id: text(path['id'], 'path id'),
    from: text(path['from'], 'path from'),
    to: text(path['to'], 'path to'),
    condition: text(path['condition'], 'path condition'),
  };
}

function readProblem(value: unknown): Problem {
  const problem = record(value, 'problem');
  return {
    code: text(problem['code'], 'problem code'),
    message: text(problem['message'], 'problem message'),
    nodeId: optionalText(problem['nodeId'], 'problem nodeId'),
    edgeId: optionalText(problem['edgeId'], 'problem edgeId'),
  };
}

/** A `GET`/`PUT /api/v1/agents/{id}/plan` body. */
function readPlan(body: unknown): AgentPlan {
  const plan = record(body, 'plan');
  const validation = record(plan['validation'], 'validation');
  return {
    saved: plan['saved'] === true,
    lockedTexts: list(plan['lockedTexts'], 'lockedTexts').map((item) =>
      text(record(item, 'locked text')['text'], 'locked text'),
    ),
    nodes: list(plan['nodes'], 'nodes').map(readStep),
    edges: list(plan['edges'], 'edges').map(readPath),
    valid: validation['valid'] === true,
    errors: list(validation['errors'], 'errors').map(readProblem),
  };
}

async function createAgent(
  request: APIRequestContext,
  name: string,
  useCase: string,
): Promise<string> {
  const response = await request.post('/api/v1/agents', {
    data: { name, useCase, voice: 'af_heart' },
  });
  expect(response.status()).toBe(201);
  return text(record(await response.json(), 'agent')['id'], 'agent id');
}

async function getPlan(request: APIRequestContext, agentId: string): Promise<AgentPlan> {
  const response = await request.get(`/api/v1/agents/${agentId}/plan`);
  expect(response.status()).toBe(200);
  return readPlan(await response.json());
}

async function putPlan(
  request: APIRequestContext,
  agentId: string,
  plan: { nodes: Step[]; edges: Path[] },
): Promise<AgentPlan> {
  const response = await request.put(`/api/v1/agents/${agentId}/plan`, {
    data: { nodes: plan.nodes, edges: plan.edges },
  });
  expect(response.status()).toBe(200);
  return readPlan(await response.json());
}

async function percentOf(request: APIRequestContext, agentId: string): Promise<unknown> {
  const response = await request.get(`/api/v1/agents/${agentId}/progress`);
  expect(response.status()).toBe(200);
  return record(await response.json(), 'progress')['percent'];
}

/** Waits until the stored plan passes `check`, within the autosave budget, and answers it. */
async function autosaved(
  request: APIRequestContext,
  agentId: string,
  check: (plan: AgentPlan) => boolean,
): Promise<AgentPlan> {
  const seen: { plan: AgentPlan | null } = { plan: null };
  await expect
    .poll(
      async () => {
        seen.plan = await getPlan(request, agentId);
        return check(seen.plan);
      },
      { timeout: AUTOSAVE_BUDGET_MS, intervals: [100] },
    )
    .toBe(true);
  return seen.plan ?? fail('plan', seen);
}

/** A step on the canvas: a group named by its label. */
function stepNamed(page: Page, label: string): Locator {
  return page.getByRole('group', { name: label, exact: true });
}

function only<T>(items: T[], what: string): T {
  return items.length === 1 && items[0] !== undefined ? items[0] : fail(what, items);
}

function idOf(plan: AgentPlan, label: string): string {
  return only(
    plan.nodes.filter((node) => node.label === label),
    `step ${label}`,
  ).id;
}

function codesOn(plan: AgentPlan, nodeId: string): string[] {
  return plan.errors
    .filter((problem) => problem.nodeId === nodeId)
    .map((problem) => problem.code)
    .toSorted();
}

/** Every problem the plan has on `nodeId` is written on its step. */
async function expectProblemsShown(step: Locator, plan: AgentPlan, nodeId: string) {
  const problems = plan.errors.filter((problem) => problem.nodeId === nodeId);
  await Promise.all(problems.map((problem) => expect(step).toContainText(problem.message)));
}

/** Every one of `steps` is on the canvas. */
async function expectSteps(page: Page, steps: Step[]) {
  await Promise.all(steps.map((step) => expect(stepNamed(page, step.label)).toBeVisible()));
}

/** Adds a path from `from` to the step labelled `to`, by keyboard. */
async function addPath(page: Page, from: Locator, to: string, when: string): Promise<void> {
  await from.getByRole('button', { name: 'Add path', exact: true }).press('Enter');
  const dialog = page.getByRole('dialog', { name: /^Add path/ });
  const target = dialog.getByLabel('To', { exact: true });
  await expect(target).toBeFocused();
  await auditScreen(page);
  await target.selectOption({ label: to });
  const condition = dialog.getByLabel('When', { exact: true });
  await condition.fill(when);
  await condition.press('Enter');
  await expect(dialog).toBeHidden();
}

test.describe('plan v1', { tag: '@contract' }, () => {
  test('an agent created from each template has a valid plan that makes it 50 % set up', async ({
    request,
  }) => {
    await Promise.all(
      USE_CASES.map(async (useCase) => {
        const agentId = await createAgent(request, `Template ${useCase}`, useCase);
        const template = await getPlan(request, agentId);
        expect(template.saved, useCase).toBe(false);
        expect(template.errors, useCase).toEqual([]);
        expect(template.valid, useCase).toBe(true);
        expect(await percentOf(request, agentId), useCase).toBe(25);

        const saved = await putPlan(request, agentId, template);

        expect(saved.saved, useCase).toBe(true);
        expect(saved.valid, useCase).toBe(true);
        expect(saved.nodes, useCase).toEqual(template.nodes);
        expect(saved.edges, useCase).toEqual(template.edges);
        expect(await percentOf(request, agentId), useCase).toBe(50);
      }),
    );
  });

  test(
    'edit the plan by keyboard: add, connect and delete a step',
    { tag: '@golden' },
    async ({ page, request }) => {
      const agentId = await createAgent(request, 'Plan desk', 'booking');
      const template = await getPlan(request, agentId);
      const start = only(
        template.nodes.filter((node) => node.kind === 'start'),
        'start step',
      );
      const end = template.nodes.find((node) => node.kind === 'end') ?? fail('end step', template);
      const added = 'Quick goodbye';
      const labels = template.nodes.map((node) => node.label);
      expect(new Set(labels).size, 'step labels are unique').toBe(labels.length);
      expect(labels).not.toContain(added);

      // From the agent's page to its Plan step.
      await page.goto(`/agents/${agentId}/provision`);
      const progress = page.getByRole('progressbar', { name: 'Setup progress' });
      await expect(progress).toHaveAttribute('aria-valuenow', '25');
      await page.getByRole('link', { name: 'Plan', exact: true }).first().press('Enter');
      await expect(page).toHaveURL(new RegExp(`/agents/${agentId}/plan$`));

      // The template, valid and not saved yet: every step on the canvas, one primary action.
      await expectSteps(page, template.nodes);
      await expect(progress).toHaveAttribute('aria-valuenow', '25');
      const use = page.getByRole('button', { name: 'Use this plan', exact: true });
      await expect(use).toHaveAttribute('data-variant', 'primary');
      await auditScreen(page);

      const status = page.getByRole('status', { name: 'Save status' });
      await use.press('Enter');
      await autosaved(request, agentId, (plan) => plan.saved);
      await expect(status).toHaveText('Saved');
      await expect(progress).toHaveAttribute('aria-valuenow', '50');
      await expect(use).toHaveCount(0);

      // Add a step. Nothing leads to it and it leads nowhere, and the step says so.
      await page.getByRole('button', { name: 'Add step', exact: true }).press('Enter');
      const addStep = page.getByRole('dialog', { name: 'Add step' });
      const name = addStep.getByLabel('Name', { exact: true });
      await expect(name).toBeFocused();
      await auditScreen(page);
      await addStep.getByLabel('Kind', { exact: true }).selectOption({ label: 'Say' });
      await name.fill(added);
      await name.press('Enter');
      await expect(addStep).toBeHidden();
      const lonely = await autosaved(request, agentId, (plan) =>
        plan.nodes.some((node) => node.label === added),
      );
      const step = stepNamed(page, added);
      await expect(step).toBeVisible();
      const stepId = idOf(lonely, added);
      expect(codesOn(lonely, stepId)).toEqual(['missing_exit', 'unreachable_node']);
      await expectProblemsShown(step, lonely, stepId);
      await expect(status).toHaveText('Saved');
      await expect(progress).toHaveAttribute('aria-valuenow', '25');

      // Connect it in, from the start step: it can be reached, but still leads nowhere.
      await addPath(page, stepNamed(page, start.label), added, 'The caller only wants to say bye');
      const reached = await autosaved(request, agentId, (plan) =>
        plan.edges.some((edge) => edge.from === start.id && edge.to === stepId),
      );
      expect(codesOn(reached, stepId)).toEqual(['missing_exit']);
      await expectProblemsShown(step, reached, stepId);

      // Connect it out, to an end step: the plan is valid again.
      await addPath(page, step, end.label, 'The caller has said goodbye');
      const connected = await autosaved(request, agentId, (plan) => plan.valid);
      expect(connected.errors).toEqual([]);
      expect(connected.edges).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            from: start.id,
            to: stepId,
            condition: 'The caller only wants to say bye',
          }),
          expect.objectContaining({
            from: stepId,
            to: end.id,
            condition: 'The caller has said goodbye',
          }),
        ]),
      );
      await Promise.all(
        lonely.errors.map((problem) => expect(step).not.toContainText(problem.message)),
      );
      await expect(status).toHaveText('Saved');
      await expect(progress).toHaveAttribute('aria-valuenow', '50');
      await auditScreen(page);

      // Delete it, after a confirmation: its paths go with it.
      await step.getByRole('button', { name: 'Delete step', exact: true }).press('Enter');
      const confirm = page.getByRole('alertdialog', { name: new RegExp(added) });
      await expect(confirm).toBeVisible();
      await auditScreen(page);
      await confirm.getByRole('button', { name: 'Delete', exact: true }).press('Enter');
      const final = await autosaved(request, agentId, (plan) =>
        plan.nodes.every((node) => node.id !== stepId),
      );
      await expect(step).toHaveCount(0);
      expect(final.nodes).toEqual(template.nodes);
      expect(final.edges).toEqual(template.edges);
      expect(final.valid).toBe(true);

      // End state: the plan survives a reload, valid, saved and at 50 %.
      await page.reload();
      await expectSteps(page, template.nodes);
      await expect(stepNamed(page, added)).toHaveCount(0);
      await expect(status).toHaveText('Saved');
      await expect(progress).toHaveAttribute('aria-valuenow', '50');
      await expect(use).toHaveCount(0);
      await auditScreen(page);
    },
  );

  test('a saved plan shows each problem on the step it belongs to', async ({ page, request }) => {
    const agentId = await createAgent(request, 'Plan problems', 'support');
    const template = await getPlan(request, agentId);
    const labels: Record<string, string> = {
      start: 'Greeting',
      ask: 'Ask what they need',
      orphan: 'Goodbye nobody hears',
    };
    const seeded = await putPlan(request, agentId, {
      nodes: [
        {
          id: 'start',
          kind: 'start',
          label: 'Greeting',
          text: [...template.lockedTexts, 'Hi.'].join(' '),
        },
        { id: 'ask', kind: 'ask', label: 'Ask what they need', text: 'What do you need?' },
        { id: 'orphan', kind: 'end', label: 'Goodbye nobody hears', text: 'Goodbye.' },
      ],
      edges: [{ id: 'to-ask', from: 'start', to: 'ask', condition: '' }],
    });
    expect(seeded.errors.map((problem) => problem.code).toSorted()).toEqual([
      'empty_condition',
      'missing_exit',
      'unreachable_node',
    ]);

    await page.goto(`/agents/${agentId}/plan`);

    const checks = seeded.errors.flatMap((problem) => {
      const path = seeded.edges.find((edge) => edge.id === problem.edgeId);
      const owner = problem.nodeId ?? path?.from ?? fail('located problem', problem);
      return Object.entries(labels).map(([nodeId, label]) => {
        const step = expect(stepNamed(page, label));
        return nodeId === owner
          ? step.toContainText(problem.message)
          : step.not.toContainText(problem.message);
      });
    });
    await Promise.all(checks);
    await expect(page.getByRole('progressbar', { name: 'Setup progress' })).toHaveAttribute(
      'aria-valuenow',
      '25',
    );
    await expect(page.getByRole('button', { name: 'Use this plan', exact: true })).toHaveCount(0);
    await auditScreen(page);
  });
});
