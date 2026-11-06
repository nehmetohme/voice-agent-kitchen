import { useId } from 'react';

import { callSummaryOf } from '../state/call-summary';
import { useTestCall } from '../state/use-test-call';
import { CallControls } from './call-controls';
import { CallNotices } from './call-notices';
import { CallStatus } from './call-status';
import { CallSummary } from './call-summary';
import { CallTranscript } from './call-transcript';
import { LatencyStrip } from './latency-strip';

/**
 * A live test call with the agent in the browser: start, talk, interrupt it, hang up. The
 * header holds the state and the controls, next to the transcript they act on (FITTS-7); the
 * transcript fills the rest and scrolls on its own, with the latency of every call below it.
 * When a call ends, its summary (turns, reply latency p50/p95, the next steps) sits under the
 * header. The panel fills its parent's height; stacked on a narrow screen, it takes one screen.
 */
export function TestCallPanel({ agentId }: { agentId: string }) {
  const call = useTestCall(agentId);
  const { state } = call;
  const titleId = useId();
  const summary = callSummaryOf(state);
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-3 rounded-md border border-line p-4 max-lg:h-dvh lg:min-h-0 lg:flex-1"
    >
      <div>
        <div className="flex items-center justify-between gap-3">
          <div className="flex min-h-target min-w-0 flex-col justify-center">
            <h2 id={titleId} className="text-lg font-semibold">
              Test call
            </h2>
            <CallStatus state={state} />
          </div>
          <CallControls call={call} />
        </div>
        <CallNotices state={state} />
        {summary !== null && <CallSummary agentId={agentId} summary={summary} />}
      </div>
      <CallTranscript lines={state.lines} hearing={state.hearing} />
      <LatencyStrip />
    </section>
  );
}
