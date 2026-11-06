import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';

import type { PurposeDetails, PurposeSettings } from '../api/purpose-models';
import {
  duplicatePurpose,
  getPurpose,
  listPurposes,
  listTools,
  savePurposeVersion,
  updatePurpose,
} from '../api/purposes-api';

/** Query keys: the list, one purpose, and the tool catalog. */
const purposeKeys = {
  all: ['purposes'] as const,
  list: () => [...purposeKeys.all, 'list'] as const,
  detail: (id: string) => [...purposeKeys.all, 'detail', id] as const,
  tools: () => ['catalog', 'tools'] as const,
};

/** Built-in purposes first, then the workspace's own, newest first. */
export const purposesQueryOptions = () =>
  queryOptions({ queryKey: purposeKeys.list(), queryFn: listPurposes });

export const purposeQueryOptions = (id: string) =>
  queryOptions({ queryKey: purposeKeys.detail(id), queryFn: () => getPurpose(id) });

/** The catalog is deployment configuration: fetch it once per session. */
export const toolCatalogQueryOptions = () =>
  queryOptions({ queryKey: purposeKeys.tools(), queryFn: listTools, staleTime: Infinity });

/** Copies a purpose; the new one's page opens without a fetch. */
export function useDuplicatePurpose() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ source, name }: { source: string; name: string }) =>
      duplicatePurpose(source, name),
    onSuccess: (created: PurposeDetails) => {
      queryClient.setQueryData(purposeKeys.detail(created.id), created);
      void queryClient.invalidateQueries({ queryKey: purposeKeys.list() });
    },
  });
}

/** Saves changed sections; the answer (with its problems) replaces the cached purpose. */
export function useUpdatePurpose(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (changes: Partial<PurposeSettings>) => updatePurpose(id, changes),
    onSuccess: (saved: PurposeDetails) => {
      queryClient.setQueryData(purposeKeys.detail(id), saved);
      void queryClient.invalidateQueries({ queryKey: purposeKeys.list() });
    },
  });
}

/** Freezes the draft as the next version. */
export function useSaveVersion(id: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => savePurposeVersion(id),
    onSuccess: (saved: PurposeDetails) => {
      queryClient.setQueryData(purposeKeys.detail(id), saved);
      void queryClient.invalidateQueries({ queryKey: purposeKeys.list() });
    },
  });
}

/** Re-reads the purpose, e.g. after its flow was saved by the flow editor. */
export function useRefreshPurpose(id: string): () => void {
  const queryClient = useQueryClient();
  return useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: purposeKeys.detail(id) });
    void queryClient.invalidateQueries({ queryKey: purposeKeys.list() });
  }, [id, queryClient]);
}
