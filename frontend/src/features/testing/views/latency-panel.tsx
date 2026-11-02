import { useQuery } from '@tanstack/react-query';
import { useId } from 'react';

import { Button } from '../../../shared/ui/button';
import { voiceLatencyQueryOptions, type VoiceStage } from '../state/latency-queries';
import { LatencyHistogram } from './latency-histogram';

const STAGE_LABEL: Record<VoiceStage, string> = {
  stt: 'Speech to text',
  decision: 'Opt-out check',
  llm_ttft: 'Model, first token',
  tts_first_audio: 'Voice, first audio',
  mouth_to_ear: 'End to end',
};

function milliseconds(value: number | null): string {
  return value === null ? '–' : `${value.toLocaleString('en-US')} ms`;
}

function LatencyTable() {
  const latency = useQuery(voiceLatencyQueryOptions());
  if (latency.isPending) {
    return (
      <p aria-busy="true" className="text-muted">
        Loading latency…
      </p>
    );
  }
  if (latency.isError) {
    return (
      <div role="alert" className="flex flex-col items-start gap-3">
        <p>Couldn't load latency. Check your connection, then try again.</p>
        <Button
          onClick={() => {
            void latency.refetch();
          }}
        >
          Try again
        </Button>
      </div>
    );
  }
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
        {latency.data.map((stage) => (
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

/** How long each stage of the voice path takes, over every test call since the server started. */
export function LatencyPanel() {
  const titleId = useId();
  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-3">
      <h2 id={titleId} className="text-lg font-semibold">
        Latency
      </h2>
      <p className="text-muted">
        Every test call since the server started. End to end runs from when you stop talking to the
        agent's first sound.
      </p>
      <LatencyTable />
    </section>
  );
}
