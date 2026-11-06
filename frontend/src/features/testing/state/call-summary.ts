// The end of a test call (peak-end, PE-1): how many turns it had and how fast the agent
// answered, as the call's own p50 and p95. Pure.
import type { TestCallState } from './test-call';

export interface CallSummary {
  turns: number;
  /** End to end (your last word to the agent's first sound), over this call's replies. */
  p50Ms: number | null;
  p95Ms: number | null;
}

/** The nearest-rank percentile of `sorted` (ascending), or `null` when it is empty. */
export function percentile(sorted: readonly number[], fraction: number): number | null {
  if (sorted.length === 0) return null;
  const rank = Math.max(1, Math.ceil(fraction * sorted.length));
  return sorted[Math.min(rank, sorted.length) - 1] ?? null;
}

/** The summary of an ended call that had something said; `null` otherwise. */
export function callSummaryOf(state: TestCallState): CallSummary | null {
  if (state.phase !== 'ended' || state.lines.length === 0) return null;
  const replies = state.lines
    .flatMap((line) => (line.replyMs === null ? [] : [line.replyMs]))
    .toSorted((a, b) => a - b);
  return {
    turns: state.lines.length,
    p50Ms: percentile(replies, 0.5),
    p95Ms: percentile(replies, 0.95),
  };
}
