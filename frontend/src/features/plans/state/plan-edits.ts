/**
 * Edits to a plan draft: pure functions that return a new draft and never change the old one.
 * The server checks the graph rules; these keep the draft well formed (unique IDs, no path
 * left pointing at a deleted step).
 */
import type { LockedText, Path, PlanDraft, Step, StepKind } from '../api/plans-api';

/** `<prefix>-<n>` with the smallest `n` (from 1) no ID in `taken` uses. */
export function nextId(prefix: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  let n = 1;
  while (used.has(`${prefix}-${n}`)) n += 1;
  return `${prefix}-${n}`;
}

/** Adds a step at the end; answers the draft and the new step's ID. */
export function addStep(
  draft: PlanDraft,
  step: { label: string; kind: StepKind },
): { draft: PlanDraft; stepId: string } {
  const id = nextId(
    'step',
    draft.steps.map((s) => s.id),
  );
  const added: Step = { id, kind: step.kind, label: step.label.trim(), text: '' };
  return { draft: { ...draft, steps: [...draft.steps, added] }, stepId: id };
}

/** Changes a step's label or text. */
export function updateStep(
  draft: PlanDraft,
  id: string,
  changes: Partial<Pick<Step, 'label' | 'text'>>,
): PlanDraft {
  return {
    ...draft,
    steps: draft.steps.map((step) => (step.id === id ? { ...step, ...changes } : step)),
  };
}

/** Removes a step and every path that leaves or reaches it. */
export function deleteStep(draft: PlanDraft, id: string): PlanDraft {
  return {
    steps: draft.steps.filter((step) => step.id !== id),
    paths: draft.paths.filter((path) => path.from !== id && path.to !== id),
  };
}

/** Adds a path at the end. */
export function addPath(draft: PlanDraft, path: Omit<Path, 'id'>): PlanDraft {
  const id = nextId(
    'path',
    draft.paths.map((p) => p.id),
  );
  return {
    ...draft,
    paths: [...draft.paths, { id, ...path, condition: path.condition.trim() }],
  };
}

export function deletePath(draft: PlanDraft, id: string): PlanDraft {
  return { ...draft, paths: draft.paths.filter((path) => path.id !== id) };
}

/**
 * Splits a start step's text into the rule pack's locked opening and the greeting after it.
 * `intact` is false when the text doesn't open with the locked texts in order (for example
 * after the jurisdiction changed the pack); then the whole text counts as the greeting.
 */
export function splitOpening(
  text: string,
  locked: readonly LockedText[],
): { intact: boolean; greeting: string } {
  let rest = text.trimStart();
  for (const { text: required } of locked) {
    if (!rest.startsWith(required)) return { intact: false, greeting: text.trim() };
    rest = rest.slice(required.length).trimStart();
  }
  return { intact: true, greeting: rest.trim() };
}

/** A start step's text: the locked texts in order, then the greeting. */
export function joinOpening(locked: readonly LockedText[], greeting: string): string {
  return [...locked.map(({ text }) => text), greeting.trim()].filter(Boolean).join(' ');
}

/** Whether two drafts hold the same steps and paths, in the same order. */
export function sameDraft(a: PlanDraft, b: PlanDraft): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}
