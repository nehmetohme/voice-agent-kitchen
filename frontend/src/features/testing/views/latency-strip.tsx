import { useQuery } from '@tanstack/react-query';

import { Button } from '../../../shared/ui/button';
import { DetailsPopover } from '../../../shared/ui/details-popover';
import { voiceLatencyQueryOptions, type StageLatency } from '../state/latency-queries';
import { milliseconds, STAGE_LABEL } from './latency-stages';
import { LatencyTable } from './latency-table';

/** One figure per stage: the median, with the 95th percentile on hover and for screen readers. */
function Figures({ stages }: { stages: readonly StageLatency[] }) {
  return (
    <dl className="grid grid-cols-5 gap-3">
      {stages.map((stage) => (
        <div key={stage.stage} className="flex min-w-0 flex-col">
          <dt className="truncate text-sm text-muted">{STAGE_LABEL[stage.stage]}</dt>
          <dd
            className="font-medium tabular-nums"
            title={`95th percentile: ${milliseconds(stage.p95Ms)}`}
          >
            {milliseconds(stage.p50Ms)}
            <span className="sr-only">, 95th percentile {milliseconds(stage.p95Ms)}</span>
          </dd>
        </div>
      ))}
    </dl>
  );
}

function Details({ stages }: { stages: readonly StageLatency[] }) {
  return (
    <DetailsPopover triggerLabel="Details" title="Latency">
      <p className="text-sm text-muted">
        Every test call since the server started. First token is the model's first word, first audio
        the voice's first sound; end to end runs from when you stop talking to the agent's first
        sound.
      </p>
      <LatencyTable stages={stages} />
    </DetailsPopover>
  );
}

/**
 * How long each stage of the voice path takes, over every test call since the server started:
 * a compact strip under the transcript, with the full table and histograms in "Details", which
 * opens over the page instead of pushing it.
 */
export function LatencyStrip() {
  const latency = useQuery(voiceLatencyQueryOptions());
  return (
    <section aria-label="Latency" className="flex flex-col border-t border-line">
      <div className="flex min-h-target items-center justify-between gap-3">
        <p className="text-sm">
          <span className="font-medium">Latency</span>
          <span className="text-muted"> · median of every test call</span>
        </p>
        {latency.isSuccess && <Details stages={latency.data} />}
      </div>
      {latency.isPending && (
        <p aria-busy="true" className="text-sm text-muted">
          Loading latency…
        </p>
      )}
      {latency.isError && (
        <div role="alert" className="flex items-center gap-2 text-sm">
          <p>Couldn't load latency.</p>
          <Button
            variant="ghost"
            onClick={() => {
              void latency.refetch();
            }}
          >
            Try again
          </Button>
        </div>
      )}
      {latency.isSuccess && <Figures stages={latency.data} />}
    </section>
  );
}
