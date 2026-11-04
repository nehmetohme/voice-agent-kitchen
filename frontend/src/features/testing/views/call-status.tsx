import { cn } from '../../../shared/lib/cn';
import type { CallPhase, TestCallState } from '../state/test-call';

/** Announced politely, once per phase: never on each turn, which you already hear. */
const PHASE: Record<CallPhase, string> = {
  idle: '',
  connecting: 'Connecting…',
  live: 'Live',
  ending: 'Hanging up…',
  // The notice under the header says how the call ended, or why it couldn't start.
  ended: '',
  failed: '',
};

/** Who is speaking on a live call right now; shown, not announced. */
function activity(state: TestCallState): string | null {
  if (state.phase !== 'live') return null;
  if (state.agentSpeaking) return 'Agent speaking';
  return state.muted ? 'Muted' : 'Listening';
}

/**
 * The call's state under its title. The phase answers every click at once (Doherty) and is a
 * polite live region; who is speaking changes every turn, so it is shown but not announced.
 */
export function CallStatus({ state }: { state: TestCallState }) {
  const phase = PHASE[state.phase];
  const now = activity(state);
  return (
    <p className="flex min-w-0 items-center gap-2 text-sm text-muted">
      {phase !== '' && (
        <span
          aria-hidden
          className={cn(
            'size-2 shrink-0 rounded-full',
            state.phase === 'live' ? 'bg-success' : 'bg-warning',
          )}
        />
      )}
      <span aria-live="polite" aria-busy={state.phase === 'connecting'} className="font-medium">
        {phase}
      </span>
      {now !== null && <span className="truncate">{now}</span>}
    </p>
  );
}
