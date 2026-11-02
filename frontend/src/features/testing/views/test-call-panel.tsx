import { useId } from 'react';

import { Button } from '../../../shared/ui/button';
import {
  isInCall,
  type CallFailure,
  type CallPhase,
  type EndReason,
  type ServerErrorCode,
  type TestCallState,
} from '../state/test-call';
import { useTestCall } from '../state/use-test-call';
import { CallTranscript } from './call-transcript';

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

function liveText(state: TestCallState): string {
  const now = state.agentSpeaking ? 'Agent speaking' : state.muted ? 'Muted' : 'Listening';
  if (state.lastReplyMs === null) return `${now}.`;
  return `${now}. Last reply took ${state.lastReplyMs.toLocaleString('en-US')} ms.`;
}

const STATUS: Record<CallPhase, (state: TestCallState) => string> = {
  idle: () => '',
  connecting: () => 'Connecting…',
  live: liveText,
  ending: () => 'Hanging up…',
  ended: (state) => (state.endReason === null ? 'Call ended.' : ENDED[state.endReason]),
  failed: () => '',
};

function CallControls({ call }: { call: ReturnType<typeof useTestCall> }) {
  const { state } = call;
  if (!isInCall(state.phase)) {
    return (
      <Button onClick={call.start}>
        {state.phase === 'idle' ? 'Start test call' : 'Call again'}
      </Button>
    );
  }
  return (
    <div className="flex gap-3">
      <Button onClick={call.end} disabled={state.phase === 'ending'}>
        End call
      </Button>
      <Button
        aria-pressed={state.muted}
        disabled={state.phase !== 'live'}
        onClick={() => {
          call.setMuted(!state.muted);
        }}
      >
        Mute
      </Button>
    </div>
  );
}

/**
 * A live test call with the agent in the browser: start, talk, interrupt it, hang up. The
 * status line answers every click at once (Doherty), and "End call" sits next to the
 * transcript (FITTS-7).
 */
export function TestCallPanel({ agentId }: { agentId: string }) {
  const call = useTestCall(agentId);
  const { state } = call;
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col items-start gap-4 border-t border-line pt-6"
    >
      <h2 id={titleId} className="text-lg font-semibold">
        Test call
      </h2>
      <p className="text-muted">
        Talk to this agent through your microphone. Wear headphones, so it doesn't hear itself.
      </p>
      <CallControls call={call} />
      <p aria-live="polite" aria-busy={state.phase === 'connecting'} className="text-muted">
        {STATUS[state.phase](state)}
      </p>
      {state.failure !== null && (
        <p role="alert" className="text-danger">
          {failureText(state.failure)}
        </p>
      )}
      {state.serverError !== null && state.phase === 'live' && (
        <p role="alert" className="text-danger">
          {SERVER_ERROR[state.serverError]}
        </p>
      )}
      <CallTranscript lines={state.lines} hearing={state.hearing} />
    </section>
  );
}
