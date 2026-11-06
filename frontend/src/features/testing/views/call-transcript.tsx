import { AudioLines, Headphones } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../../shared/lib/cn';
import { FollowScroll } from '../../../shared/ui/follow-scroll';
import type { TranscriptLine } from '../state/test-call';

interface CallTranscriptProps {
  lines: readonly TranscriptLine[];
  /** What the caller is saying right now. */
  hearing: string | null;
  /** Who the caller is: "You" on a test call, "Caller" in a test run. */
  callerLabel?: string | undefined;
  /** What shows before the first line (default: how to talk to the agent). */
  empty?: ReactNode;
}

interface BubbleProps {
  speaker: TranscriptLine['speaker'];
  callerLabel: string;
  /** Not final yet: what the caller is saying, or the agent still speaking. */
  pending?: boolean | undefined;
  /** How long the agent took to answer, end to end. */
  replyMs?: number | null | undefined;
  children: ReactNode;
}

/** One line, chat style: the agent outlined on the left, you filled on the right. */
function Bubble({ speaker, callerLabel, pending = false, replyMs = null, children }: BubbleProps) {
  const agent = speaker === 'agent';
  return (
    <li className={cn('flex flex-col gap-1', agent ? 'items-start pr-12' : 'items-end pl-12')}>
      <span className="px-1 text-sm text-muted">
        <span className="font-medium">{agent ? 'Agent' : callerLabel}</span>
        {replyMs !== null && (
          <span
            className="tabular-nums"
            title="End to end: from when you stopped talking to the agent's first sound"
          >
            {' · '}
            <span className="sr-only">replied in </span>
            {replyMs.toLocaleString('en-US')} ms
          </span>
        )}
        <span className="sr-only">:</span>
      </span>
      <p
        className={cn(
          'max-w-readable rounded-md px-3 py-2',
          agent ? 'border border-line bg-canvas' : 'bg-caller',
          pending && 'text-muted',
        )}
      >
        {children}
      </p>
    </li>
  );
}

function Speaking() {
  return (
    <span className="inline-flex items-center gap-2">
      <AudioLines aria-hidden className="size-4" />
      Speaking…
    </span>
  );
}

/** Before the first line: how to talk to the agent. */
function NothingSaidYet() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center text-muted">
      <Headphones aria-hidden className="size-6" />
      <p>Talk to this agent through your microphone.</p>
      <p>Wear headphones, so it doesn't hear itself.</p>
    </div>
  );
}

/**
 * The conversation in turn order, in its own scroll region that follows the newest line. The
 * agent's line appears as soon as it starts speaking and fills in when it finishes; the
 * caller's words show as they are heard. Not a live region: you already hear every line.
 */
export function CallTranscript({
  lines,
  hearing,
  callerLabel = 'You',
  empty,
}: CallTranscriptProps) {
  return (
    <FollowScroll label="Transcript">
      {lines.length === 0 && hearing === null ? (
        (empty ?? <NothingSaidYet />)
      ) : (
        <ol className="flex flex-col gap-3 p-1">
          {lines.map((line) => (
            <Bubble
              key={line.key}
              speaker={line.speaker}
              callerLabel={callerLabel}
              pending={line.text === null}
              replyMs={line.replyMs}
            >
              {line.text ?? <Speaking />}
              {line.interrupted && <span className="text-muted"> (interrupted)</span>}
            </Bubble>
          ))}
          {hearing !== null && (
            <Bubble speaker="caller" callerLabel={callerLabel} pending>
              {hearing}…
            </Bubble>
          )}
        </ol>
      )}
    </FollowScroll>
  );
}
