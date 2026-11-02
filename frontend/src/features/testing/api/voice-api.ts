/**
 * The testing feature's door to the backend: starting a test call and reading the latency
 * histograms. Wraps the generated client and maps its DTOs and errors to view models.
 */
import { startSession } from '../../../shared/api/generated/endpoints/sessions/sessions';
import { getVoiceLatency as getVoiceLatencyRequest } from '../../../shared/api/generated/endpoints/voice/voice';
import type { VoiceStage } from '../../../shared/api/generated/model';
import { ApiError } from '../../../shared/api/http';

export type { VoiceStage };

/** A prepared call: its media socket path, usable once within a minute. */
export interface CallTicket {
  mediaPath: string;
}

/** One histogram bucket: replies that took at most `leMs` (`null`: the slowest bucket). */
export interface LatencyBucket {
  leMs: number | null;
  count: number;
}

export interface StageLatency {
  stage: VoiceStage;
  count: number;
  p50Ms: number | null;
  p95Ms: number | null;
  buckets: LatencyBucket[];
}

/** Why a call couldn't start. */
export type StartFailure =
  /** The server said no (voice not set up, too many calls); `detail` says why. */
  | { kind: 'refused'; detail: string }
  /** The agent is gone or archived. */
  | { kind: 'missing' }
  /** Anything else: network, server error. */
  | { kind: 'connection' };

export async function startTestCall(agentId: string): Promise<CallTicket> {
  const session = await startSession(agentId);
  return { mediaPath: session.mediaUrl };
}

/** Every stage of the voice path, in pipeline order. */
export async function getVoiceLatency(): Promise<StageLatency[]> {
  const latency = await getVoiceLatencyRequest();
  return latency.stages.map((stage) => ({
    stage: stage.stage,
    count: stage.count,
    p50Ms: stage.p50Ms ?? null,
    p95Ms: stage.p95Ms ?? null,
    buckets: stage.buckets.map((bucket) => ({ leMs: bucket.leMs ?? null, count: bucket.count })),
  }));
}

/** Reads an error from `startTestCall`. */
export function startFailureOf(error: unknown): StartFailure {
  if (!(error instanceof ApiError)) return { kind: 'connection' };
  if (error.status === 404 || error.status === 409) return { kind: 'missing' };
  if (error.status === 503 && error.problem.detail !== undefined) {
    return { kind: 'refused', detail: error.problem.detail };
  }
  return { kind: 'connection' };
}
