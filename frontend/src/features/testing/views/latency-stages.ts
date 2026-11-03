import type { VoiceStage } from '../state/latency-queries';

/** One name per stage, the same in the strip and in its details (similarity). */
export const STAGE_LABEL: Record<VoiceStage, string> = {
  stt: 'Speech to text',
  decision: 'Opt-out check',
  llm_ttft: 'First token',
  tts_first_audio: 'First audio',
  mouth_to_ear: 'End to end',
};

/** "1,250 ms", or a dash before the first reply. */
export function milliseconds(value: number | null): string {
  return value === null ? '–' : `${value.toLocaleString('en-US')} ms`;
}
