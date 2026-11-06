/**
 * The purposes feature's door to the backend: wraps the generated client and maps its DTOs to
 * the view models state and views use, so a contract change touches this folder only.
 */
import { listTools as listToolsRequest } from '../../../shared/api/generated/endpoints/catalog/catalog';
import {
  createPurpose as createPurposeRequest,
  getPurpose as getPurposeRequest,
  listPurposes as listPurposesRequest,
  savePurposeVersion as savePurposeVersionRequest,
  updatePurpose as updatePurposeRequest,
} from '../../../shared/api/generated/endpoints/purposes/purposes';
import { Problem, UpdatePurposeBody } from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';
import { sectionsBody, toDetails } from './purpose-mapping';
import type {
  PurposeDetails,
  PurposeListItem,
  PurposeSettings,
  ToolCatalog,
} from './purpose-models';

export async function listPurposes(): Promise<PurposeListItem[]> {
  const list = await listPurposesRequest();
  return list.items.map((item) => ({
    id: item.id,
    name: item.name,
    description: item.description,
    complianceClass: item.complianceClass,
    system: item.system,
    latestVersion: item.latestVersion,
    unsavedChanges: item.unsavedChanges,
    usedBy: item.usedBy,
  }));
}

export async function getPurpose(id: string): Promise<PurposeDetails> {
  return toDetails(await getPurposeRequest(id));
}

/** Copies `source` (its latest version) into a new draft named `name`. */
export async function duplicatePurpose(source: string, name: string): Promise<PurposeDetails> {
  return toDetails(await createPurposeRequest({ duplicateOf: source, name }));
}

/** Saves the changed sections (autosave); the answer carries the draft's problems. */
export async function updatePurpose(
  id: string,
  changes: Partial<PurposeSettings>,
): Promise<PurposeDetails> {
  const body = sectionsBody(changes);
  UpdatePurposeBody.parse(body);
  return toDetails(await updatePurposeRequest(id, body));
}

export async function savePurposeVersion(id: string): Promise<PurposeDetails> {
  return toDetails(await savePurposeVersionRequest(id));
}

export async function listTools(): Promise<ToolCatalog> {
  const catalog = await listToolsRequest();
  return {
    builtins: catalog.builtins.map((tool) => ({ ...tool })),
    allowedHosts: catalog.allowedHosts,
  };
}

/** The `{field, message}` pairs of a 422 problem; empty for any other error. */
export function fieldErrorsOf(error: unknown): { field: string; message: string }[] {
  if (!(error instanceof ApiError) || error.status !== 422) return [];
  const problem = Problem.safeParse(error.problem);
  return problem.success ? (problem.data.errors ?? []) : [];
}

/** The problem's detail, for people; `undefined` when there is none. */
export function problemDetail(error: unknown): string | undefined {
  if (!(error instanceof ApiError)) return undefined;
  const problem = Problem.safeParse(error.problem);
  return problem.success ? problem.data.detail : undefined;
}

/** True when the server says the purpose doesn't exist. */
export function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.status === 404;
}
