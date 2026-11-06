/**
 * DTO ↔ view model mapping of a purpose: the server's draft to the editor's sections, and the
 * changed sections back to an `updatePurpose` body (each section replaces the stored one whole).
 */
import { UpdatePurposeBody, type Purpose as PurposeDto } from '../../../shared/api/generated/model';
import type { PurposeDetails, PurposeSettings } from './purpose-models';

export function toDetails(dto: PurposeDto): PurposeDetails {
  const { draft } = dto;
  return {
    id: dto.id,
    system: dto.system,
    latestVersion: dto.latestVersion,
    unsavedChanges: dto.unsavedChanges,
    usedBy: dto.usedBy,
    settings: {
      overview: {
        name: draft.name,
        description: draft.description,
        complianceClass: draft.complianceClass,
        persona: draft.persona,
      },
      tools: {
        builtins: [...draft.tools.builtins],
        http: draft.tools.http.map((tool) => ({
          name: tool.name,
          description: tool.description,
          method: tool.method,
          url: tool.url,
          timeoutMs: tool.timeoutMs,
          secret: tool.secret ?? '',
          confirm: tool.confirm,
          inputs: Object.entries(tool.inputs.properties).map(([name, property]) => ({
            name,
            type: property.type,
            description: property.description,
            required: tool.inputs.required.includes(name),
          })),
        })),
      },
      guardrails: {
        allowedTopics: [...draft.guardrails.allowedTopics],
        never: [...draft.guardrails.never],
        escalations: draft.guardrails.escalations.map((rule) => ({
          when: rule.when,
          action: rule.action,
          message: rule.message ?? '',
        })),
        maxCallMinutes: draft.guardrails.maxCallMinutes,
        neverAsk: [...draft.guardrails.neverAsk],
      },
      compliance: { ...draft.compliance },
    },
    problems: dto.problems.map(({ code, message, field }) => ({ code, message, field })),
    baselines: dto.baselines.map((baseline) => ({
      jurisdiction: baseline.jurisdiction,
      rulePack: `${baseline.rulePack.id} v${String(baseline.rulePack.version)}`,
      lockedTexts: baseline.lockedTexts.map(({ text }) => text),
      optOutPhrases: baseline.optOutPhrases,
      optOutConfirmation: baseline.optOutConfirmation,
      forbiddenClaims: baseline.forbiddenClaims,
      retention: baseline.retention,
      effective: baseline.effective,
    })),
  };
}

/** The request body of the changed sections: each section replaces the stored one whole. */
export function sectionsBody(changes: Partial<PurposeSettings>): UpdatePurposeBody {
  const body: UpdatePurposeBody = {};
  if (changes.overview !== undefined) Object.assign(body, changes.overview);
  if (changes.tools !== undefined) {
    body.tools = {
      builtins: changes.tools.builtins,
      http: changes.tools.http.map((tool) => ({
        name: tool.name,
        description: tool.description,
        method: tool.method,
        url: tool.url,
        timeoutMs: tool.timeoutMs,
        confirm: tool.confirm,
        ...(tool.secret.trim() === '' ? {} : { secret: tool.secret.trim() }),
        inputs: {
          type: 'object',
          properties: Object.fromEntries(
            tool.inputs.map((input) => [
              input.name,
              { type: input.type, description: input.description },
            ]),
          ),
          required: tool.inputs.filter((input) => input.required).map((input) => input.name),
        },
      })),
    };
  }
  if (changes.guardrails !== undefined) {
    body.guardrails = {
      ...changes.guardrails,
      escalations: changes.guardrails.escalations.map((rule) => ({
        when: rule.when,
        action: rule.action,
        ...(rule.action === 'end_call' ? { message: rule.message } : {}),
      })),
    };
  }
  if (changes.compliance !== undefined) body.compliance = changes.compliance;
  return body;
}

/** Whether the changed sections can be sent: every field within the contract's schema. */
export function isSendable(changes: Partial<PurposeSettings>): boolean {
  return UpdatePurposeBody.safeParse(sectionsBody(changes)).success;
}
