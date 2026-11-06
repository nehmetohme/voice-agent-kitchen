// Editing the scenario set: the form a dialog edits, its checks, and the set's own rules. Pure;
// the limits come from the contract's generated schema (`SCENARIO_LIMITS`).
import { SCENARIO_LIMITS, type RubricItem, type Scenario } from '../api/test-runs-api';

/** What the scenario dialog edits. The turn cap is the input's text. */
export interface ScenarioForm {
  name: string;
  goal: string;
  persona: string;
  required: boolean;
  maxTurns: string;
  checks: RubricItem[];
}

export interface FormErrors {
  name?: string | undefined;
  goal?: string | undefined;
  persona?: string | undefined;
  maxTurns?: string | undefined;
  /** One per check, in order. */
  checks?: (string | undefined)[] | undefined;
}

export type FormResult = { ok: true; scenario: Scenario } | { ok: false; errors: FormErrors };

const DEFAULT_MAX_TURNS = 8;

/** A new scenario: required when nothing else is, one check expecting yes. */
export function newScenarioForm(existing: readonly Scenario[]): ScenarioForm {
  return {
    name: '',
    goal: '',
    persona: '',
    required: !existing.some((scenario) => scenario.required),
    maxTurns: String(DEFAULT_MAX_TURNS),
    checks: [{ question: '', expected: 'yes' }],
  };
}

export function formOf(scenario: Scenario): ScenarioForm {
  return {
    name: scenario.name,
    goal: scenario.goal,
    persona: scenario.persona,
    required: scenario.required,
    maxTurns: String(scenario.maxTurns),
    checks: scenario.rubric.map((item) => ({ ...item })),
  };
}

/** The form with one more check, up to the contract's limit. */
export function withCheck(form: ScenarioForm): ScenarioForm {
  if (form.checks.length >= SCENARIO_LIMITS.rubric) return form;
  return { ...form, checks: [...form.checks, { question: '', expected: 'yes' }] };
}

/** The form without check `index`; the last check stays. */
export function withoutCheck(form: ScenarioForm, index: number): ScenarioForm {
  if (form.checks.length <= 1) return form;
  return { ...form, checks: form.checks.filter((_, at) => at !== index) };
}

function lengthError(text: string, max: number, what: string): string | undefined {
  if (text.trim() === '') return `Write the ${what}.`;
  return text.trim().length > max ? `Keep the ${what} to ${max} characters.` : undefined;
}

function errorsOf(form: ScenarioForm): FormErrors {
  const turns = Number(form.maxTurns);
  const checks = form.checks.map((check) =>
    lengthError(check.question, SCENARIO_LIMITS.question, 'question'),
  );
  return {
    name: lengthError(form.name, SCENARIO_LIMITS.name, 'name'),
    goal: lengthError(form.goal, SCENARIO_LIMITS.goal, "caller's goal"),
    persona:
      form.persona.trim().length > SCENARIO_LIMITS.persona
        ? `Keep the persona to ${SCENARIO_LIMITS.persona} characters.`
        : undefined,
    maxTurns:
      Number.isInteger(turns) && turns >= 1 && turns <= SCENARIO_LIMITS.maxTurns
        ? undefined
        : `Allow 1 to ${SCENARIO_LIMITS.maxTurns} caller turns.`,
    checks: checks.some((error) => error !== undefined) ? checks : undefined,
  };
}

/** The scenario the form describes, with `id`, or why it can't be one. */
export function scenarioFromForm(form: ScenarioForm, id: string): FormResult {
  const errors = errorsOf(form);
  if (Object.values(errors).some((error) => error !== undefined)) return { ok: false, errors };
  return {
    ok: true,
    scenario: {
      id,
      name: form.name.trim(),
      goal: form.goal.trim(),
      persona: form.persona.trim(),
      required: form.required,
      maxTurns: Number(form.maxTurns),
      rubric: form.checks.map((check) => ({
        question: check.question.trim(),
        expected: check.expected,
      })),
    },
  };
}

/** A new ID from the name: lowercase words joined by hyphens, unique in the set. */
export function newScenarioId(name: string, taken: readonly string[]): string {
  const words = name
    .toLowerCase()
    .replaceAll(/[^a-z0-9]+/g, '-')
    .replaceAll(/^-+|-+$/g, '');
  const base = (/^[a-z]/.test(words) ? words : `scenario-${words}`).slice(0, 32).replace(/-+$/, '');
  if (!taken.includes(base)) return base;
  for (let n = 2; ; n += 1) {
    const candidate = `${base}-${n}`;
    if (!taken.includes(candidate)) return candidate;
  }
}

/** The set with `scenario` in place of the one with its ID, or added at the end. */
export function withScenario(scenarios: readonly Scenario[], scenario: Scenario): Scenario[] {
  const at = scenarios.findIndex((other) => other.id === scenario.id);
  if (at === -1) return [...scenarios, scenario];
  return scenarios.map((other, index) => (index === at ? scenario : other));
}

export function withoutScenario(scenarios: readonly Scenario[], id: string): Scenario[] {
  return scenarios.filter((scenario) => scenario.id !== id);
}

/** Why the set can't be saved as it is: the server's set rules, said for people. */
export function setProblem(scenarios: readonly Scenario[]): string | null {
  if (scenarios.length === 0) return 'Keep at least one scenario.';
  if (scenarios.length > SCENARIO_LIMITS.scenarios) {
    return `Keep at most ${SCENARIO_LIMITS.scenarios} scenarios.`;
  }
  if (!scenarios.some((scenario) => scenario.required)) {
    return 'Keep at least one scenario required: a run passes only if every required one does.';
  }
  return null;
}
