/**
 * The contract's request limits for a purpose, taken from the generated schemas so the forms
 * can't drift from the server. Rules the schema doesn't carry (hosts on the allow-list,
 * repeats) are the server's: its answer names them.
 */
import {
  updatePurposeInputComplianceForbiddenClaimsMax,
  updatePurposeInputComplianceOptOutPhrasesMax,
  updatePurposeInputComplianceRecordingsDaysMax,
  updatePurposeInputDescriptionMax,
  updatePurposeInputGuardrailsAllowedTopicsMax,
  updatePurposeInputGuardrailsEscalationsItemMessageMax,
  updatePurposeInputGuardrailsEscalationsItemWhenMax,
  updatePurposeInputGuardrailsEscalationsMax,
  updatePurposeInputGuardrailsMaxCallMinutesMax,
  updatePurposeInputGuardrailsNeverMax,
  updatePurposeInputNameMax,
  updatePurposeInputPersonaMax,
  updatePurposeInputToolsHttpItemDescriptionMax,
  updatePurposeInputToolsHttpItemInputsPropertiesDescriptionMax,
  updatePurposeInputToolsHttpItemTimeoutMsDefault,
  updatePurposeInputToolsHttpItemTimeoutMsMax,
  updatePurposeInputToolsHttpItemTimeoutMsMin,
  updatePurposeInputToolsHttpItemUrlMax,
  updatePurposeInputToolsHttpMax,
} from '../../../shared/api/generated/model';

export const LIMITS = {
  name: updatePurposeInputNameMax,
  description: updatePurposeInputDescriptionMax,
  persona: updatePurposeInputPersonaMax,
  httpTools: updatePurposeInputToolsHttpMax,
  toolDescription: updatePurposeInputToolsHttpItemDescriptionMax,
  inputDescription: updatePurposeInputToolsHttpItemInputsPropertiesDescriptionMax,
  url: updatePurposeInputToolsHttpItemUrlMax,
  timeoutMs: {
    min: updatePurposeInputToolsHttpItemTimeoutMsMin,
    max: updatePurposeInputToolsHttpItemTimeoutMsMax,
    default: updatePurposeInputToolsHttpItemTimeoutMsDefault,
  },
  inputs: 10,
  topics: updatePurposeInputGuardrailsAllowedTopicsMax,
  never: updatePurposeInputGuardrailsNeverMax,
  item: 200,
  escalations: updatePurposeInputGuardrailsEscalationsMax,
  question: updatePurposeInputGuardrailsEscalationsItemWhenMax,
  message: updatePurposeInputGuardrailsEscalationsItemMessageMax,
  callMinutes: updatePurposeInputGuardrailsMaxCallMinutesMax,
  phrases: updatePurposeInputComplianceOptOutPhrasesMax,
  phrase: 100,
  claims: updatePurposeInputComplianceForbiddenClaimsMax,
  days: updatePurposeInputComplianceRecordingsDaysMax,
} as const;
