/**
 * The agents feature's door to the backend: wraps the generated client and maps its DTOs to
 * the view models state and views use, so a contract change touches this file only.
 */
import {
  archiveAgent as archiveAgentRequest,
  createAgent as createAgentRequest,
  getAgent as getAgentRequest,
  getAgentProgress as getAgentProgressRequest,
  listAgents as listAgentsRequest,
  updateAgent as updateAgentRequest,
} from '../../../shared/api/generated/endpoints/agents/agents';
import {
  listLanguages as listLanguagesRequest,
  listVoices as listVoicesRequest,
} from '../../../shared/api/generated/endpoints/catalog/catalog';
import {
  CreateAgentBody,
  Problem,
  UpdateAgentBody,
  type Agent as AgentDto,
  type ModelProfile,
  type SetupStep,
  type UseCase,
} from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';

export type { ModelProfile, SetupStep, UseCase };

/** An agent as the list shows it. */
export interface AgentListItem {
  id: string;
  name: string;
  useCase: UseCase;
  updatedAt: string;
  progressPercent: number;
}

interface TurnDetection {
  silenceMs: number;
  bargeIn: boolean;
}

/** Everything the provision page edits. */
export interface AgentSettings {
  name: string;
  useCase: UseCase;
  voice: string;
  language: string;
  jurisdiction: string;
  model: ModelProfile;
  turnDetection: TurnDetection;
}

export interface AgentDetails {
  id: string;
  archived: boolean;
  settings: AgentSettings;
}

/** The three fields creating an agent needs; the server defaults the rest. */
export interface NewAgent {
  name: string;
  useCase: UseCase;
  voice: string;
}

export interface SetupProgress {
  percent: number;
  steps: { step: SetupStep; done: boolean }[];
  nextStep: SetupStep | null;
}

/** One entry of a catalog picker. */
export interface Choice {
  value: string;
  label: string;
}

/** Field name (as the API names it) → what to fix. */
export type FieldErrors = Partial<Record<keyof AgentSettings, string>>;

function toDetails(dto: AgentDto): AgentDetails {
  return {
    id: dto.id,
    archived: dto.archivedAt !== null,
    settings: {
      name: dto.name,
      useCase: dto.useCase,
      voice: dto.voice,
      language: dto.language,
      jurisdiction: dto.jurisdiction,
      model: dto.model,
      turnDetection: { ...dto.turnDetection },
    },
  };
}

export async function listAgents(): Promise<AgentListItem[]> {
  const list = await listAgentsRequest();
  return list.items.map((item) => ({
    id: item.id,
    name: item.name,
    useCase: item.useCase,
    updatedAt: item.updatedAt,
    progressPercent: item.progressPercent,
  }));
}

export async function getAgent(id: string): Promise<AgentDetails> {
  return toDetails(await getAgentRequest(id));
}

// Request bodies are checked against the contract before they leave (Postel, POS-4): a body
// the schema rejects throws here instead of reaching the server.

export async function createAgent(agent: NewAgent): Promise<AgentDetails> {
  CreateAgentBody.parse(agent);
  return toDetails(await createAgentRequest(agent));
}

export async function updateAgent(
  id: string,
  changes: Partial<AgentSettings>,
): Promise<AgentDetails> {
  UpdateAgentBody.parse(changes);
  return toDetails(await updateAgentRequest(id, changes));
}

export async function archiveAgent(id: string): Promise<void> {
  await archiveAgentRequest(id);
}

export async function getAgentProgress(id: string): Promise<SetupProgress> {
  const progress = await getAgentProgressRequest(id);
  return {
    percent: progress.percent,
    steps: progress.steps.map(({ step, done }) => ({ step, done })),
    nextStep: progress.nextStep,
  };
}

export async function listVoices(): Promise<Choice[]> {
  const voices = await listVoicesRequest();
  return voices.items.map((voice) => ({
    value: voice.id,
    label: `${voice.name} · ${voice.description}`,
  }));
}

export async function listLanguages(): Promise<Choice[]> {
  const languages = await listLanguagesRequest();
  return languages.items.map((language) => ({ value: language.code, label: language.name }));
}

const SETTINGS_FIELDS: readonly (keyof AgentSettings)[] = [
  'name',
  'useCase',
  'voice',
  'language',
  'jurisdiction',
  'model',
  'turnDetection',
];

function isSettingsField(field: string): field is keyof AgentSettings {
  return SETTINGS_FIELDS.some((known) => known === field);
}

/** The per-field messages of a 422 problem; empty for any other error. */
export function fieldErrorsOf(error: unknown): FieldErrors {
  if (!(error instanceof ApiError) || error.status !== 422) return {};
  const problem = Problem.safeParse(error.problem);
  const errors: FieldErrors = {};
  for (const { field, message } of problem.success ? (problem.data.errors ?? []) : []) {
    if (isSettingsField(field)) errors[field] = message;
  }
  return errors;
}

/** True when the server says the resource doesn't exist. */
export function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
}
