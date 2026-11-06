import { queryOptions } from '@tanstack/react-query';

import {
  getAgent,
  getAgentProgress,
  listAgents,
  listLanguages,
  listVoices,
} from '../api/agents-api';
import { getAgentPurpose, listPurposeChoices } from '../api/purpose-api';

/** Query keys: the list, one agent and its progress, and the catalog. */
export const agentKeys = {
  all: ['agents'] as const,
  list: () => [...agentKeys.all, 'list'] as const,
  detail: (id: string) => [...agentKeys.all, 'detail', id] as const,
  progress: (id: string) => [...agentKeys.all, 'progress', id] as const,
  purpose: (id: string) => [...agentKeys.all, 'purpose', id] as const,
  purposeChoices: () => ['purposes', 'choices'] as const,
  voices: () => ['catalog', 'voices'] as const,
  languages: () => ['catalog', 'languages'] as const,
};

/** Active agents, most recently updated first. */
export const agentsQueryOptions = () =>
  queryOptions({ queryKey: agentKeys.list(), queryFn: listAgents });

export const agentQueryOptions = (id: string) =>
  queryOptions({ queryKey: agentKeys.detail(id), queryFn: () => getAgent(id) });

/** Setup progress comes from the server, so it survives a reload (ZEIG-2). */
export const agentProgressQueryOptions = (id: string) =>
  queryOptions({ queryKey: agentKeys.progress(id), queryFn: () => getAgentProgress(id) });

/** The catalog is static per deployment: fetch it once per session. */
export const voicesQueryOptions = () =>
  queryOptions({ queryKey: agentKeys.voices(), queryFn: listVoices, staleTime: Infinity });

export const languagesQueryOptions = () =>
  queryOptions({ queryKey: agentKeys.languages(), queryFn: listLanguages, staleTime: Infinity });

/**
 * The purpose version the agent pins, and the update to a newer one. Purposes change on their
 * own page, so this is read again whenever the agent's page opens.
 */
export const agentPurposeQueryOptions = (id: string) =>
  queryOptions({
    queryKey: agentKeys.purpose(id),
    queryFn: () => getAgentPurpose(id),
    staleTime: 0,
  });

/** The purposes an agent can pin (those with a saved version), read again on each picker. */
export const purposeChoicesQueryOptions = () =>
  queryOptions({
    queryKey: agentKeys.purposeChoices(),
    queryFn: listPurposeChoices,
    staleTime: 0,
  });
