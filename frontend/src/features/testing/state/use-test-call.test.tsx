// useTestCall with a fake connector: the call's life from start to hang-up, the failure paths,
// and the races (hanging up while connecting, leaving the page mid-call).
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { ApiError } from '../../../shared/api/http';
import type { ServerEvent } from '../../../shared/voice/media-protocol';
import type { VoiceSession, VoiceSessionHandlers } from '../../../shared/voice/voice-session';
import { AGENT_ID } from '../../../test/agents-api';
import { MEDIA_PATH } from '../../../test/voice-api';
import type { CallTicket } from '../api/voice-api';
import { latencyKeys } from './latency-queries';
import { useTestCall, type CallConnector } from './use-test-call';

const started: ServerEvent = {
  type: 'session.started',
  session_id: 'abc',
  audio: {
    input: { encoding: 'pcm_s16le', sample_rate: 16_000, frame_ms: 20 },
    output: { encoding: 'pcm_s16le', sample_rate: 24_000, frame_ms: 20 },
  },
};

function deferred<T>() {
  const settle: { resolve?: (value: T) => void; reject?: (error: unknown) => void } = {};
  const promise = new Promise<T>((resolve, reject) => {
    Object.assign(settle, { resolve, reject });
  });
  return {
    promise,
    resolve: (value: T) => settle.resolve?.(value),
    reject: (error: unknown) => settle.reject?.(error),
  };
}

/** A connector whose session the test drives: server events in, calls recorded. */
function fakeConnector(ticket: Promise<CallTicket> = Promise.resolve({ mediaPath: MEDIA_PATH })) {
  const line: { handlers?: VoiceSessionHandlers } = {};
  const session = { setMuted: vi.fn<(muted: boolean) => void>(), end: vi.fn<() => void>() };
  const connector = {
    start: vi.fn<(agentId: string) => Promise<CallTicket>>(() => ticket),
    open: vi.fn<(path: string, handlers: VoiceSessionHandlers) => Promise<VoiceSession>>(
      (_path, handlers) => {
        line.handlers = handlers;
        return Promise.resolve(session);
      },
    ),
  } satisfies CallConnector;
  return {
    connector,
    session,
    emit: (event: ServerEvent) => {
      act(() => {
        line.handlers?.onEvent(event);
      });
    },
    close: () => {
      act(() => {
        line.handlers?.onClosed();
      });
    },
  };
}

function renderCall(connector: CallConnector) {
  const queryClient = new QueryClient();
  const invalidate = vi.spyOn(queryClient, 'invalidateQueries');
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
  const rendered = renderHook(() => useTestCall(AGENT_ID, connector), { wrapper });
  return { ...rendered, invalidate };
}

describe('useTestCall', () => {
  it('runs a call from start to hang-up, refreshing latency per turn', async () => {
    const fake = fakeConnector();
    const { result, invalidate } = renderCall(fake.connector);

    act(() => {
      result.current.start();
    });
    expect(result.current.state.phase).toBe('connecting');
    await waitFor(() => {
      expect(fake.connector.open).toHaveBeenCalledWith(MEDIA_PATH, expect.anything());
    });
    expect(fake.connector.start).toHaveBeenCalledWith(AGENT_ID);

    fake.emit(started);
    expect(result.current.state.phase).toBe('live');
    fake.emit({ type: 'agent.audio.start', turn_id: 1 });
    fake.emit({
      type: 'latency',
      turn_id: 1,
      stt_ms: 90,
      llm_ttft_ms: 210,
      tts_first_audio_ms: 120,
      mouth_to_ear_ms: 640,
    });
    expect(result.current.state.lines).toMatchObject([{ key: 'agent-1', replyMs: 640 }]);
    expect(invalidate).toHaveBeenCalledWith({ queryKey: latencyKeys.all });

    act(() => {
      result.current.setMuted(true);
    });
    expect(fake.session.setMuted).toHaveBeenCalledWith(true);
    expect(result.current.state.muted).toBe(true);

    act(() => {
      result.current.end();
    });
    expect(fake.session.end).toHaveBeenCalledOnce();
    expect(result.current.state.phase).toBe('ending');
    fake.close();
    expect(result.current.state.phase).toBe('ended');
    expect(invalidate).toHaveBeenCalledTimes(2);
  });

  it('fails with the server reason when the call is refused', async () => {
    const fake = fakeConnector(
      Promise.reject(new ApiError(503, { detail: 'Too many test calls are running.' })),
    );
    const { result } = renderCall(fake.connector);

    act(() => {
      result.current.start();
    });

    await waitFor(() => {
      expect(result.current.state.failure).toEqual({
        kind: 'refused',
        detail: 'Too many test calls are running.',
      });
    });
    expect(fake.connector.open).not.toHaveBeenCalled();
  });

  it('fails as a microphone problem when the session cannot open', async () => {
    const fake = fakeConnector();
    fake.connector.open.mockRejectedValue(new DOMException('denied', 'NotAllowedError'));
    const { result } = renderCall(fake.connector);

    act(() => {
      result.current.start();
    });

    await waitFor(() => {
      expect(result.current.state.failure).toEqual({ kind: 'microphone' });
    });
  });

  it('hangs up a call that was still connecting as soon as it opens', async () => {
    const ticket = deferred<CallTicket>();
    const fake = fakeConnector(ticket.promise);
    const { result } = renderCall(fake.connector);

    act(() => {
      result.current.start();
      result.current.end();
    });
    await act(async () => {
      ticket.resolve({ mediaPath: MEDIA_PATH });
      await ticket.promise;
    });

    await waitFor(() => {
      expect(fake.session.end).toHaveBeenCalledOnce();
    });
    fake.close();
    expect(result.current.state.phase).toBe('ended');
  });

  it('treats a failure after hanging up as the end of the call', async () => {
    const ticket = deferred<CallTicket>();
    const fake = fakeConnector(ticket.promise);
    const { result } = renderCall(fake.connector);

    act(() => {
      result.current.start();
      result.current.end();
    });
    await act(async () => {
      ticket.reject(new TypeError('Failed to fetch'));
      await ticket.promise.catch(() => undefined);
    });

    await waitFor(() => {
      expect(result.current.state.phase).toBe('ended');
    });
    expect(result.current.state.failure).toBeNull();
  });

  it('ends a live call when the page goes away', async () => {
    const fake = fakeConnector();
    const { result, unmount } = renderCall(fake.connector);
    act(() => {
      result.current.start();
    });
    await waitFor(() => {
      expect(fake.connector.open).toHaveBeenCalled();
    });

    unmount();

    expect(fake.session.end).toHaveBeenCalledOnce();
  });
});
