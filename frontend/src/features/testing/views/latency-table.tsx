import type { StageLatency } from '../state/latency-queries';
import { LatencyHistogram } from './latency-histogram';
import { milliseconds, STAGE_LABEL } from './latency-stages';

/** Every stage with its median, 95th percentile, reply count and spread. */
export function LatencyTable({ stages }: { stages: readonly StageLatency[] }) {
  return (
    <table className="w-full text-left text-sm">
      <thead className="text-muted">
        <tr>
          <th scope="col" className="py-2 font-medium">
            Stage
          </th>
          <th scope="col" className="py-2 font-medium">
            Median
          </th>
          <th scope="col" className="py-2 font-medium">
            95th percentile
          </th>
          <th scope="col" className="py-2 font-medium">
            Replies
          </th>
          <th scope="col" className="py-2 font-medium">
            Spread
          </th>
        </tr>
      </thead>
      <tbody>
        {stages.map((stage) => (
          <tr key={stage.stage} className="border-t border-line">
            <th scope="row" className="py-2 font-medium">
              {STAGE_LABEL[stage.stage]}
            </th>
            <td className="py-2">{milliseconds(stage.p50Ms)}</td>
            <td className="py-2">{milliseconds(stage.p95Ms)}</td>
            <td className="py-2">{stage.count.toLocaleString('en-US')}</td>
            <td className="py-2">
              <LatencyHistogram buckets={stage.buckets} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
