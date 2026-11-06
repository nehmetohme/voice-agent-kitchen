/**
 * The agent's purpose: which purposes an agent can pin, the version it pins, and the update to
 * a newer one. Wraps the generated client like `agents-api.ts`.
 */
import {
  getAgentPurpose as getAgentPurposeRequest,
  updateAgentPurpose as updateAgentPurposeRequest,
} from '../../../shared/api/generated/endpoints/agents/agents';
import { listPurposes as listPurposesRequest } from '../../../shared/api/generated/endpoints/purposes/purposes';
import type {
  AgentPurpose as AgentPurposeDto,
  PlanOnUpdate,
  PurposeChangeArea,
  UseCase,
} from '../../../shared/api/generated/model';

/**
 * A purpose an agent can pin: one with a saved version. `value` is what the pickers send:
 * a built-in purpose's use case (as the create form always has), or a purpose's ID.
 */
export interface PurposeChoice {
  value: string;
  label: string;
  useCase: UseCase;
  system: boolean;
}

export interface PurposeUpdate {
  version: number;
  changes: { area: PurposeChangeArea; summary: string }[];
  /** What happens to the plan unless the operator replaces an edited one. */
  plan: PlanOnUpdate;
  planEdited: boolean;
}

export interface AgentPurpose {
  id: string;
  version: number;
  name: string;
  system: boolean;
  latestVersion: number | null;
  update: PurposeUpdate | null;
}

/** The picker value of a pinned purpose: its use case when built-in, its ID otherwise. */
export function purposeKey(purpose: { id: string; system: boolean }, useCase: UseCase): string {
  return purpose.system ? useCase : purpose.id;
}

/** The purposes an agent can pin, built-in ones first. */
export async function listPurposeChoices(): Promise<PurposeChoice[]> {
  const list = await listPurposesRequest();
  return list.items
    .filter((item) => item.latestVersion !== null)
    .map((item) => ({
      value: item.system ? item.complianceClass : item.id,
      label: item.name,
      useCase: item.complianceClass,
      system: item.system,
    }));
}

function toAgentPurpose(dto: AgentPurposeDto): AgentPurpose {
  return {
    id: dto.id,
    version: dto.version,
    name: dto.name,
    system: dto.system,
    latestVersion: dto.latestVersion,
    update:
      dto.update === null
        ? null
        : {
            version: dto.update.version,
            changes: dto.update.changes.map(({ area, summary }) => ({ area, summary })),
            plan: dto.update.plan,
            planEdited: dto.update.planEdited,
          },
  };
}

export async function getAgentPurpose(agentId: string): Promise<AgentPurpose> {
  return toAgentPurpose(await getAgentPurposeRequest(agentId));
}

/** Moves the agent to `version`; an edited plan is kept unless `replacePlan`. */
export async function updateAgentPurpose(
  agentId: string,
  version: number,
  replacePlan: boolean,
): Promise<AgentPurpose> {
  return toAgentPurpose(await updateAgentPurposeRequest(agentId, { version, replacePlan }));
}
