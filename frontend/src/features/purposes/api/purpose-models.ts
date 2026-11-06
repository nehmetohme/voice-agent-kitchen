/** The purposes feature's view models: what state and views work with. */
import type {
  BuiltinTool,
  EscalationAction,
  HttpMethod,
  PersonalData,
  ToolInputType,
  UseCase,
} from '../../../shared/api/generated/model';

export type { BuiltinTool, EscalationAction, HttpMethod, PersonalData, ToolInputType, UseCase };

/** A purpose as the list shows it. */
export interface PurposeListItem {
  id: string;
  name: string;
  description: string;
  complianceClass: UseCase;
  system: boolean;
  latestVersion: number | null;
  unsavedChanges: boolean;
  usedBy: number;
}

export interface ToolInput {
  name: string;
  type: ToolInputType;
  description: string;
  required: boolean;
}

/** A custom HTTP tool as the editor holds it; `secret` is a name, `''` for none. */
export interface HttpToolDraft {
  name: string;
  description: string;
  method: HttpMethod;
  url: string;
  timeoutMs: number;
  secret: string;
  confirm: boolean;
  inputs: ToolInput[];
}

export interface Escalation {
  when: string;
  action: EscalationAction;
  /** Said before ending the call; `''` for other actions. */
  message: string;
}

export interface OverviewSettings {
  name: string;
  description: string;
  complianceClass: UseCase;
  persona: string;
}

export interface ToolSettings {
  builtins: BuiltinTool[];
  http: HttpToolDraft[];
}

export interface GuardrailSettings {
  allowedTopics: string[];
  never: string[];
  escalations: Escalation[];
  maxCallMinutes: number;
  neverAsk: PersonalData[];
}

export interface ComplianceSettings {
  optOutPhrases: string[];
  forbiddenClaims: string[];
  recordingsDays: number | null;
  transcriptsDays: number | null;
}

/** Everything the purpose page edits, by section. */
export interface PurposeSettings {
  overview: OverviewSettings;
  tools: ToolSettings;
  guardrails: GuardrailSettings;
  compliance: ComplianceSettings;
}

export type SectionKey = keyof PurposeSettings;

export interface PurposeProblem {
  code: string;
  message: string;
  field: string;
}

interface Retention {
  recordingsDays: number;
  transcriptsDays: number;
  auditDays: number;
}

/** One jurisdiction's rule pack for the purpose's class, and the policy with its additions. */
export interface Baseline {
  jurisdiction: string;
  rulePack: string;
  lockedTexts: string[];
  optOutPhrases: string[];
  optOutConfirmation: string;
  forbiddenClaims: string[];
  retention: Retention;
  effective: { optOutPhrases: string[]; forbiddenClaims: string[]; retention: Retention };
}

export interface PurposeDetails {
  id: string;
  system: boolean;
  latestVersion: number | null;
  unsavedChanges: boolean;
  usedBy: number;
  settings: PurposeSettings;
  problems: PurposeProblem[];
  baselines: Baseline[];
}

interface BuiltinToolInfo {
  name: BuiltinTool;
  label: string;
  description: string;
  locked: boolean;
}

export interface ToolCatalog {
  builtins: BuiltinToolInfo[];
  allowedHosts: string[];
}
