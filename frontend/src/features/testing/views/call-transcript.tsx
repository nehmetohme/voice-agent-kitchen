import type { TranscriptLine } from '../state/test-call';

interface CallTranscriptProps {
  lines: readonly TranscriptLine[];
  /** What the caller is saying right now. */
  hearing: string | null;
}

/** The conversation so far, oldest first, with the caller's words as they come in. */
export function CallTranscript({ lines, hearing }: CallTranscriptProps) {
  if (lines.length === 0 && hearing === null) return null;
  return (
    <ol aria-label="Transcript" className="flex w-full flex-col gap-2">
      {lines.map((line) => (
        <li key={line.key}>
          <span className="font-medium">{line.speaker === 'agent' ? 'Agent' : 'You'}: </span>
          {line.text}
          {line.interrupted && <span className="text-muted"> (interrupted)</span>}
        </li>
      ))}
      {hearing !== null && (
        <li className="text-muted">
          <span className="font-medium">You: </span>
          {hearing}…
        </li>
      )}
    </ol>
  );
}
