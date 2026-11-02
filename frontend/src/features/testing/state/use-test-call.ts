import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useReducer, useRef } from 'react';

import {
  openVoiceSession,
  type VoiceSession,
  type VoiceSessionHandlers,
} from '../../../shared/voice/voice-session';
import { startFailureOf, startTestCall, type CallTicket } from '../api/voice-api';
import { latencyKeys } from './latency-queries';
import { initialCall, testCallReducer, type TestCallState } from './test-call';

/** How a call reaches the server and the microphone; tests pass a fake. */
export interface CallConnector {
  start(agentId: string): Promise<CallTicket>;
  open(mediaPath: string, handlers: VoiceSessionHandlers): Promise<VoiceSession>;
}

const browserConnector: CallConnector = {
  start: startTestCall,
  open: (mediaPath, handlers) => openVoiceSession(mediaPath, handlers),
};

export interface TestCall {
  state: TestCallState;
  /** Call it from a click: the browser lets audio play only after a user gesture. */
  start: () => void;
  end: () => void;
  setMuted: (muted: boolean) => void;
}

/** What the hook keeps between renders: the open session and whether it is still wanted. */
interface Line {
  session: VoiceSession | null;
  wanted: boolean;
}

/** A live test call with one agent: start, mute, hang up; the latency panel refreshes per turn. */
export function useTestCall(
  agentId: string,
  connector: CallConnector = browserConnector,
): TestCall {
  const [state, dispatch] = useReducer(testCallReducer, initialCall);
  const line = useRef<Line>({ session: null, wanted: false });
  const queryClient = useQueryClient();

  const refreshLatency = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: latencyKeys.all });
  }, [queryClient]);

  const handlers: VoiceSessionHandlers = {
    onEvent(event) {
      dispatch({ type: 'event', event });
      if (event.type === 'latency') refreshLatency();
    },
    onClosed() {
      line.current.session = null;
      dispatch({ type: 'closed' });
      refreshLatency();
    },
  };

  const connect = async () => {
    let ticket: CallTicket;
    try {
      ticket = await connector.start(agentId);
    } catch (error) {
      // Hung up while connecting: the call is simply over.
      dispatch(
        line.current.wanted
          ? { type: 'failed', failure: startFailureOf(error) }
          : { type: 'closed' },
      );
      return;
    }
    try {
      const session = await connector.open(ticket.mediaPath, handlers);
      line.current.session = session;
      if (!line.current.wanted) session.end();
    } catch {
      dispatch(
        line.current.wanted
          ? { type: 'failed', failure: { kind: 'microphone' } }
          : { type: 'closed' },
      );
    }
  };

  useEffect(() => {
    const current = line.current;
    return () => {
      current.wanted = false;
      current.session?.end();
    };
  }, []);

  return {
    state,
    start: () => {
      line.current.wanted = true;
      dispatch({ type: 'start' });
      void connect();
    },
    end: () => {
      line.current.wanted = false;
      dispatch({ type: 'hang-up' });
      line.current.session?.end();
    },
    setMuted: (muted) => {
      dispatch({ type: 'mute', muted });
      line.current.session?.setMuted(muted);
    },
  };
}
