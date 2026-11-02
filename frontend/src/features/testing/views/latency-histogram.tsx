import type { LatencyBucket } from '../state/latency-queries';

/** "≤ 200 ms", or "> 5,000 ms" for the open-ended slowest bucket. */
function bucketLabel(buckets: readonly LatencyBucket[], index: number): string {
  const bound = buckets[index]?.leMs;
  if (bound !== null && bound !== undefined) return `≤ ${bound.toLocaleString('en-US')} ms`;
  const previous = buckets[index - 1]?.leMs ?? 0;
  return `> ${previous.toLocaleString('en-US')} ms`;
}

/**
 * One stage's replies per latency bucket, as columns on one baseline: a single series, so
 * one neutral colour and no legend. Screen readers get the same counts as text; the table
 * around it carries the figures.
 */
export function LatencyHistogram({ buckets }: { buckets: readonly LatencyBucket[] }) {
  const peak = Math.max(1, ...buckets.map((bucket) => bucket.count));
  const filled = buckets.flatMap((bucket, index) =>
    bucket.count > 0 ? [`${bucketLabel(buckets, index)}: ${bucket.count}`] : [],
  );
  const summary = filled.length === 0 ? 'No replies yet' : `Replies by time, ${filled.join(', ')}`;
  return (
    <>
      <span className="sr-only">{summary}</span>
      <div aria-hidden className="flex h-12 items-end gap-1 border-b border-line">
        {buckets.map((bucket, index) => (
          <div
            key={bucket.leMs ?? 'slowest'}
            title={`${bucketLabel(buckets, index)}: ${bucket.count}`}
            className="w-2 rounded-t-sm bg-muted"
            style={{ height: `${(bucket.count / peak) * 100}%` }}
          />
        ))}
      </div>
    </>
  );
}
