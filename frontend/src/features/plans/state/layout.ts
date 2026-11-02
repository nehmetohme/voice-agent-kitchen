/**
 * Where steps sit on the canvas: one column per distance from the start (a breadth-first walk
 * along the paths, in plan order), so the flow reads left to right. Steps no path reaches
 * from the start sit in a last column of their own, where their problem is easy to see. Pure,
 * so the layout never depends on screen size or on where someone last dragged a card.
 */
import type { Path, PlanDraft, Step } from '../api/plans-api';

/** The steps a path from `id` leads to, in plan order. */
function nextSteps(id: string, paths: readonly Path[], known: ReadonlySet<string>): string[] {
  return paths.filter((path) => path.from === id && known.has(path.to)).map((path) => path.to);
}

/** Each reached step's distance from a start step, in the order the walk reached them. */
function distances(draft: PlanDraft): Map<string, number> {
  const known = new Set(draft.steps.map((step) => step.id));
  const depth = new Map<string, number>();
  const queue = draft.steps.filter((step) => step.kind === 'start').map((step) => step.id);
  for (const id of queue) depth.set(id, 0);
  for (let at = 0; at < queue.length; at += 1) {
    const id = queue[at] ?? '';
    const next = (depth.get(id) ?? 0) + 1;
    for (const to of nextSteps(id, draft.paths, known)) {
      if (depth.has(to)) continue;
      depth.set(to, next);
      queue.push(to);
    }
  }
  return depth;
}

/** The steps in columns, left to right; each column top to bottom. */
export function columnsOf(draft: PlanDraft): Step[][] {
  const byId = new Map(draft.steps.map((step) => [step.id, step]));
  const depth = distances(draft);
  const columns: Step[][] = [];
  for (const [id, column] of depth) {
    const step = byId.get(id);
    if (step !== undefined) (columns[column] ??= []).push(step);
  }
  const unreached = draft.steps.filter((step) => !depth.has(step.id));
  if (unreached.length > 0) columns.push(unreached);
  return columns;
}

/** Each step's column, for drawing paths between them. */
export function columnIndex(columns: readonly Step[][]): Map<string, number> {
  const index = new Map<string, number>();
  columns.forEach((column, at) => {
    for (const step of column) index.set(step.id, at);
  });
  return index;
}
