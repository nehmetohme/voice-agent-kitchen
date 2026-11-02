import { queryOptions } from '@tanstack/react-query';

import { getVoiceLatency } from '../api/voice-api';

export type { LatencyBucket, VoiceStage } from '../api/voice-api';

export const latencyKeys = {
  all: ['voice', 'latency'] as const,
};

/** Per-stage latency histograms of every call since the server started. */
export const voiceLatencyQueryOptions = () =>
  queryOptions({ queryKey: latencyKeys.all, queryFn: getVoiceLatency });
