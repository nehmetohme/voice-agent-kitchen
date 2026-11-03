import { CircleAlert, PhoneOff } from 'lucide-react';

import type { CallFailure, EndReason, ServerErrorCode, TestCallState } from '../state/test-call';

const ENDED: Record<EndReason, string> = {
  client_ended: 'Call ended.',
  opt_out: 'The caller opted out, so the agent ended the call.',
  escalation: 'The agent handed the call to a person.',
  end_node: 'The conversation reached its end.',
  timeout: 'The call reached its time limit.',
  error: 'The call ended because of a problem on the server.',
};

const SERVER_ERROR: Record<ServerErrorCode, string> = {
  bad_message: "The browser sent something the server didn't understand.",
  unsupported_audio: "The server can't play this microphone's audio format.",
  provider_unavailable: "A model didn't answer in time. Try saying that again.",
  internal: 'Something went wrong on the server.',
};

const FAILURE: Record<Exclude<CallFailure['kind'], 'refused'>, string> = {
  missing: 'This agent no longer exists.',
  microphone:
    "Couldn't use your microphone. Allow microphone access in the browser, then try again.",
  connection: "Couldn't start the call. Check your connection, then try again.",
};

/** A refusal carries the server's own reason ("Too many test calls are running…"). */
const failureText = (failure: CallFailure) =>
  failure.kind === 'refused' ? failure.detail : FAILURE[failure.kind];

function endedText(state: TestCallState): string | null {
  if (state.phase !== 'ended') return null;
  return state.endReason === null ? 'Call ended.' : ENDED[state.endReason];
}

/** Why the call couldn't start, or the last problem the server reported while it is live. */
function problemText(state: TestCallState): string | null {
  if (state.failure !== null) return failureText(state.failure);
  if (state.serverError !== null && state.phase === 'live') return SERVER_ERROR[state.serverError];
  return null;
}

const NOTICE = 'mt-3 flex items-center gap-2 rounded-md border px-3 py-2 text-sm';

/**
 * One compact line under the call's header: how the call ended (polite), or what went wrong
 * (an alert). Nothing takes space while there is nothing to say.
 */
export function CallNotices({ state }: { state: TestCallState }) {
  const ended = endedText(state);
  const problem = problemText(state);
  return (
    <>
      {/* Always rendered, so the end of a call is announced when it arrives. */}
      <div aria-live="polite">
        {ended !== null && (
          <p className={`${NOTICE} border-line bg-surface`}>
            <PhoneOff aria-hidden className="size-4 shrink-0 text-muted" />
            {ended}
          </p>
        )}
      </div>
      {problem !== null && (
        <p role="alert" className={`${NOTICE} border-danger text-danger`}>
          <CircleAlert aria-hidden className="size-4 shrink-0" />
          {problem}
        </p>
      )}
    </>
  );
}
