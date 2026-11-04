import { Mic, MicOff, Phone, PhoneOff } from 'lucide-react';

import { Button } from '../../../shared/ui/button';
import { isInCall } from '../state/test-call';
import type { TestCall } from '../state/use-test-call';

/**
 * Start (the screen's one primary action), or End call and Mute during a call. The first button
 * stays the same element from Start to End call, so the keyboard focus stays on it.
 */
export function CallControls({ call }: { call: TestCall }) {
  const { state } = call;
  if (!isInCall(state.phase)) {
    return (
      <div data-region="actions" className="flex gap-2">
        <Button variant="primary" onClick={call.start}>
          <Phone aria-hidden className="size-4" />
          {state.phase === 'idle' ? 'Start test call' : 'Call again'}
        </Button>
      </div>
    );
  }
  return (
    <div data-region="actions" className="flex gap-2">
      <Button onClick={call.end} disabled={state.phase === 'ending'}>
        <PhoneOff aria-hidden className="size-4" />
        End call
      </Button>
      <Button
        aria-pressed={state.muted}
        disabled={state.phase !== 'live'}
        onClick={() => {
          call.setMuted(!state.muted);
        }}
      >
        {state.muted ? (
          <MicOff aria-hidden className="size-4" />
        ) : (
          <Mic aria-hidden className="size-4" />
        )}
        Mute
      </Button>
    </div>
  );
}
