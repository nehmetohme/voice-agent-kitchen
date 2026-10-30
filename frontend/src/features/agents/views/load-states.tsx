import { Link } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';

import { Button } from '../../../shared/ui/button';
import { EmptyState } from '../../../shared/ui/empty-state';

/** Shown at once while data loads (Doherty: feedback within 100 ms). */
export function Loading({ label }: { label: string }) {
  return (
    <p aria-busy="true" className="flex items-center gap-2 text-muted">
      <LoaderCircle aria-hidden className="size-4" />
      {label}
    </p>
  );
}

/** A load that failed, with the way forward (peak-end: no dead ends). */
export function LoadFailed({ what, onRetry }: { what: string; onRetry: () => void }) {
  return (
    <section
      role="alert"
      className="flex max-w-readable flex-col items-start gap-4 rounded-md border border-line bg-surface p-8"
    >
      <div className="flex flex-col gap-2">
        <h2 className="text-lg font-semibold">Couldn't load {what}</h2>
        <p className="text-muted">Check your connection, then try again.</p>
      </div>
      <Button onClick={onRetry}>Try again</Button>
    </section>
  );
}

/** An agent that doesn't exist (or no longer does), with a way back. */
export function AgentMissing() {
  return (
    <EmptyState
      title="This agent doesn't exist"
      description="It may have been archived, or the address is wrong."
      action={
        <Link to="/agents" className="font-medium underline underline-offset-4">
          Go to Agents
        </Link>
      }
    />
  );
}
