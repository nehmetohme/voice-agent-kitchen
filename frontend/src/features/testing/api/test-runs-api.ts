/**
 * The Test step's door to the backend: the agent's scenarios and its test runs. Wraps the
 * generated client and maps its DTOs to the view models state and views use, so a contract
 * change touches this file only.
 */
import {
  cancelTestRun as cancelTestRunRequest,
  getAgentScenarios,
  getTestRun as getTestRunRequest,
  listAgentTestRuns,
  saveAgentScenarios,
  startTestRun as startTestRunRequest,
} from '../../../shared/api/generated/endpoints/testing/testing';
import {
  SaveAgentScenariosBody,
  saveAgentScenariosBodyScenariosItemGoalMax,
  saveAgentScenariosBodyScenariosItemMaxTurnsMax,
  saveAgentScenariosBodyScenariosItemNameMax,
  saveAgentScenariosBodyScenariosItemPersonaMax,
  saveAgentScenariosBodyScenariosItemRubricItemQuestionMax,
  saveAgentScenariosBodyScenariosItemRubricMax,
  saveAgentScenariosBodyScenariosMax,
  type AgentScenarios as AgentScenariosDto,
  type ScenarioEndReason,
  type ScenarioOptOutLayer,
  type ScenarioStatus,
  type TestRun as TestRunDto,
  type TestRunStatus,
  type Verdict,
} from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';

export type { ScenarioStatus, TestRunStatus };

/** A yes/no question about the conversation, and the answer that passes. */
export interface RubricItem {
  question: string;
  expected: 'yes' | 'no';
}

/** What a simulated caller plays, and how its conversation is scored. */
export interface Scenario {
  id: string;
  name: string;
  goal: string;
  persona: string;
  required: boolean;
  maxTurns: number;
  rubric: RubricItem[];
}

export interface ScenarioSetView {
  /** `false` while these are the use case's defaults. */
  saved: boolean;
  scenarios: Scenario[];
}

export interface RubricCheck {
  question: string;
  expected: 'yes' | 'no';
  /** The probability of yes; `null` when the decision model gave no answer. */
  probability: number | null;
  passed: boolean;
}

interface TranscriptEntry {
  speaker: 'caller' | 'agent';
  text: string;
}

export interface ScenarioResult {
  id: string;
  name: string;
  required: boolean;
  status: ScenarioStatus;
  endReason: ScenarioEndReason | null;
  optOutLayer: ScenarioOptOutLayer | null;
  durationMs: number | null;
  transcript: TranscriptEntry[];
  checks: RubricCheck[];
}

export interface TestRunView {
  id: string;
  status: TestRunStatus;
  verdict: Verdict | null;
  draftHash: string;
  progress: { done: number; total: number; percent: number };
  models: { llm: string; decisions: string; seed: number } | null;
  scenarios: ScenarioResult[];
}

/** A run in the agent's list: is it on the current draft? */
export interface RunSummary {
  id: string;
  status: TestRunStatus;
  /** Whether it tested the agent's current draft; `false` means it is stale. */
  current: boolean;
}

/** The contract's limits, from the generated request schema. */
export const SCENARIO_LIMITS = {
  scenarios: saveAgentScenariosBodyScenariosMax,
  name: saveAgentScenariosBodyScenariosItemNameMax,
  goal: saveAgentScenariosBodyScenariosItemGoalMax,
  persona: saveAgentScenariosBodyScenariosItemPersonaMax,
  maxTurns: saveAgentScenariosBodyScenariosItemMaxTurnsMax,
  rubric: saveAgentScenariosBodyScenariosItemRubricMax,
  question: saveAgentScenariosBodyScenariosItemRubricItemQuestionMax,
} as const;

function toSet(dto: AgentScenariosDto): ScenarioSetView {
  return {
    saved: dto.saved,
    scenarios: dto.scenarios.map((scenario) => ({
      id: scenario.id,
      name: scenario.name,
      goal: scenario.goal,
      persona: scenario.persona,
      required: scenario.required,
      maxTurns: scenario.maxTurns,
      rubric: scenario.rubric.map(({ question, expected }) => ({ question, expected })),
    })),
  };
}

function toRun(dto: TestRunDto): TestRunView {
  return {
    id: dto.id,
    status: dto.status,
    verdict: dto.verdict ?? null,
    draftHash: dto.draftHash,
    progress: dto.progress,
    models: dto.models ?? null,
    scenarios: dto.scenarios.map((result) => ({
      id: result.id,
      name: result.name,
      required: result.required,
      status: result.status,
      endReason: result.endReason ?? null,
      optOutLayer: result.optOutLayer ?? null,
      durationMs: result.durationMs ?? null,
      transcript: result.transcript.map(({ speaker, text }) => ({ speaker, text })),
      checks: result.checks.map((check) => ({
        question: check.question,
        expected: check.expected,
        probability: check.probability ?? null,
        passed: check.passed,
      })),
    })),
  };
}

export async function getScenarios(agentId: string): Promise<ScenarioSetView> {
  return toSet(await getAgentScenarios(agentId));
}

/** Saves the whole set; it is checked against the contract before it leaves (POS-4). */
export async function saveScenarios(
  agentId: string,
  scenarios: readonly Scenario[],
): Promise<ScenarioSetView> {
  const body = { scenarios: [...scenarios] };
  SaveAgentScenariosBody.parse(body);
  return toSet(await saveAgentScenarios(agentId, body));
}

/** Whether `scenarios` can be sent: every field within the contract's request schema. */
export function isSendable(scenarios: readonly Scenario[]): boolean {
  return SaveAgentScenariosBody.safeParse({ scenarios: [...scenarios] }).success;
}

export async function startTestRun(agentId: string): Promise<TestRunView> {
  return toRun(await startTestRunRequest({ agentId }));
}

export async function getTestRun(id: string): Promise<TestRunView> {
  return toRun(await getTestRunRequest(id));
}

export async function cancelTestRun(id: string): Promise<TestRunView> {
  return toRun(await cancelTestRunRequest(id));
}

/** The agent's runs, newest first. */
export async function listTestRuns(agentId: string): Promise<RunSummary[]> {
  const list = await listAgentTestRuns(agentId);
  return list.items.map(({ id, status, current }) => ({ id, status, current }));
}

/** True when the server says the agent doesn't exist. */
export function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
}

/** Why a run couldn't start, in the server's words when it gave them (409, 503). */
export function startProblemOf(error: unknown): string {
  if (error instanceof ApiError && error.problem.detail !== undefined && error.status !== 500) {
    return error.problem.detail;
  }
  return "Couldn't start the test run. Check your connection, then try again.";
}
