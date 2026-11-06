/**
 * The plans feature's door to the backend: wraps the generated client and maps its DTOs to the
 * view models state and views use, so a contract change touches this file only.
 */
import {
  getAgentPlan as getAgentPlanRequest,
  saveAgentPlan as saveAgentPlanRequest,
} from '../../../shared/api/generated/endpoints/plans/plans';
import {
  getPurposeFlow as getPurposeFlowRequest,
  savePurposeFlow as savePurposeFlowRequest,
} from '../../../shared/api/generated/endpoints/purposes/purposes';
import {
  SaveAgentPlanBody,
  saveAgentPlanBodyEdgesItemConditionMax,
  saveAgentPlanBodyNodesItemLabelMax,
  saveAgentPlanBodyNodesItemTextMax,
  saveAgentPlanBodyNodesMax,
  type AgentPlan as AgentPlanDto,
  type StepKind,
} from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';

export type { StepKind };

/** A step of the plan. */
export interface Step {
  id: string;
  kind: StepKind;
  label: string;
  text: string;
}

/** A path from one step to another, taken when its condition holds. */
export interface Path {
  id: string;
  from: string;
  to: string;
  condition: string;
}

/** Text the agent says word for word, from the rule pack. */
export interface LockedText {
  key: string;
  text: string;
}

/** A rule the plan breaks, on a step, a path, or (both `null`) the whole plan. */
export interface Problem {
  code: string;
  message: string;
  stepId: string | null;
  pathId: string | null;
}

/** What the editor changes and saves. */
export interface PlanDraft {
  steps: Step[];
  paths: Path[];
}

/** Whose flow the editor edits: an agent's plan, or a purpose's starting flow. */
export type PlanTarget =
  | { kind: 'agent'; agentId: string }
  | { kind: 'purpose'; purposeId: string };

/** The agent's plan, or a purpose's starting flow, as the server sees it. */
export interface PlanView extends PlanDraft {
  /** `false` while this is the use case's template. */
  saved: boolean;
  /** What the start step's text opens with, word for word (an agent's plan). */
  lockedTexts: LockedText[];
  /**
   * What every agent's opening says before a purpose's start step, shown for information: a
   * purpose's flow doesn't carry the locked texts, since each agent's pack supplies them.
   */
  prependedTexts: LockedText[];
  optOutConfirmation: string | null;
  valid: boolean;
  problems: Problem[];
}

/** The contract's limits, from the generated request schema. */
export const PLAN_LIMITS = {
  steps: saveAgentPlanBodyNodesMax,
  label: saveAgentPlanBodyNodesItemLabelMax,
  text: saveAgentPlanBodyNodesItemTextMax,
  condition: saveAgentPlanBodyEdgesItemConditionMax,
} as const;

function toView(dto: AgentPlanDto, target: PlanTarget['kind'] = 'agent'): PlanView {
  const locked = dto.lockedTexts.map(({ key, text }) => ({ key, text }));
  return {
    saved: dto.saved,
    lockedTexts: target === 'agent' ? locked : [],
    prependedTexts: target === 'agent' ? [] : locked,
    optOutConfirmation: dto.optOutConfirmation,
    steps: dto.nodes.map(({ id, kind, label, text }) => ({ id, kind, label, text })),
    paths: dto.edges.map(({ id, from, to, condition }) => ({ id, from, to, condition })),
    valid: dto.validation.valid,
    problems: dto.validation.errors.map((error) => ({
      code: error.code,
      message: error.message,
      stepId: error.nodeId ?? null,
      pathId: error.edgeId ?? null,
    })),
  };
}

export async function getPlan(agentId: string): Promise<PlanView> {
  return toView(await getAgentPlanRequest(agentId));
}

/**
 * Saves the whole plan. The body is checked against the contract before it leaves (POS-4);
 * a plan that breaks a graph rule is still saved and comes back with its problems.
 */
export async function savePlan(agentId: string, draft: PlanDraft): Promise<PlanView> {
  const body = { nodes: draft.steps, edges: draft.paths };
  SaveAgentPlanBody.parse(body);
  return toView(await saveAgentPlanRequest(agentId, body));
}

async function getPurposeFlow(purposeId: string): Promise<PlanView> {
  return toView(await getPurposeFlowRequest(purposeId), 'purpose');
}

/** Saves a purpose's whole starting flow, checked against the contract first, like a plan. */
async function savePurposeFlow(purposeId: string, draft: PlanDraft): Promise<PlanView> {
  const body = { nodes: draft.steps, edges: draft.paths };
  SaveAgentPlanBody.parse(body);
  return toView(await savePurposeFlowRequest(purposeId, body), 'purpose');
}

/** Reads the target's flow. */
export function getFlow(target: PlanTarget): Promise<PlanView> {
  return target.kind === 'agent' ? getPlan(target.agentId) : getPurposeFlow(target.purposeId);
}

/** Saves the target's flow. */
export function saveFlow(target: PlanTarget, draft: PlanDraft): Promise<PlanView> {
  return target.kind === 'agent'
    ? savePlan(target.agentId, draft)
    : savePurposeFlow(target.purposeId, draft);
}

/** Whether `draft` can be sent: every field within the contract's request schema. */
export function isSendable(draft: PlanDraft): boolean {
  return SaveAgentPlanBody.safeParse({ nodes: draft.steps, edges: draft.paths }).success;
}

/** True when the server says the agent doesn't exist. */
export function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
}
