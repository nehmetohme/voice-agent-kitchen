import { MessagesSquare } from 'lucide-react';
import { useId } from 'react';

import type { Scenario, ScenarioResult as Result } from '../state/models';
import { STANDING_LABEL, durationText, endingText, resultOf } from '../state/run-view';
import type { TranscriptLine } from '../state/test-call';
import type { TestPageModel } from '../state/use-test-page';
import { CallTranscript } from './call-transcript';
import { RubricChecks } from './rubric-checks';

/** A run's transcript as the call transcript shows lines: one per turn, in order. */
function linesOf(result: Result | null): TranscriptLine[] {
  return (result?.transcript ?? []).map((entry, turn) => ({
    key: `${entry.speaker}-${turn}`,
    turn,
    speaker: entry.speaker,
    text: entry.text,
    interrupted: false,
    replyMs: null,
  }));
}

function NotPlayed() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center text-muted">
      <MessagesSquare aria-hidden className="size-6" />
      <p>Run the tests to see this conversation.</p>
    </div>
  );
}

/** The scenario's outcome and ending in one line, then what the caller was after. */
function Summary({ scenario, result }: { scenario: Scenario; result: Result | null }) {
  const duration = durationText(result?.durationMs ?? null);
  return (
    <div className="flex flex-col gap-1 text-sm">
      <p>
        <span className="font-medium">{STANDING_LABEL[result?.status ?? 'unrun']}</span>
        {result !== null && ` · ${endingText(result)}`}
        {duration !== null && <span className="text-muted"> · {duration}</span>}
      </p>
      <p className="line-clamp-2 text-muted" title={scenario.goal}>
        {scenario.required ? 'Required. ' : 'Optional. '}
        Goal: {scenario.goal}
      </p>
    </div>
  );
}

/**
 * The selected scenario's latest result: its outcome, the conversation (its own scroll
 * region) and the rubric checks, each passed or failed. Before a run, its rubric.
 */
export function ScenarioResult({ page }: { page: TestPageModel }) {
  const titleId = useId();
  const scenario = page.selected;
  if (scenario === null) return null;
  const result = resultOf(page.latest.run, scenario.id);
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-3 rounded-md border border-line p-4 max-lg:h-dvh lg:min-h-0 lg:flex-1"
    >
      <div className="flex flex-col gap-1">
        <h3 id={titleId} className="text-lg font-semibold">
          {scenario.name}
        </h3>
        <Summary scenario={scenario} result={result} />
      </div>
      <CallTranscript
        lines={linesOf(result)}
        hearing={null}
        callerLabel="Caller"
        empty={<NotPlayed />}
      />
      <div className="max-h-2/5 shrink-0 overflow-y-auto">
        <RubricChecks rubric={scenario.rubric} checks={result?.checks ?? []} />
      </div>
    </section>
  );
}
