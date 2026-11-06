import { Link } from '@tanstack/react-router';
import { ListChecks, Rocket } from 'lucide-react';
import { useId } from 'react';

import { Button, buttonLook } from '../../../shared/ui/button';
import type { CallSummary as Summary } from '../state/call-summary';
import { milliseconds } from './latency-stages';

interface CallSummaryProps {
  agentId: string;
  summary: Summary;
}

/**
 * The end of a test call (peak-end, PE-1): how it went, in one line (turns, this call's reply
 * latency p50 and p95), then the next steps, "Run tests" and "Publish" (M6: "Later" until
 * then). The transcript stays above it.
 */
export function CallSummary({ agentId, summary }: CallSummaryProps) {
  const laterId = useId();
  return (
    <div className="mt-3 flex flex-wrap items-center justify-between gap-3 rounded-md border border-line px-3 py-2">
      <output aria-label="Call summary" className="text-sm">
        <span className="font-medium">
          {summary.turns} {summary.turns === 1 ? 'turn' : 'turns'}
        </span>
        <span className="text-muted"> · replies </span>
        <span className="tabular-nums">
          p50 {milliseconds(summary.p50Ms)} · p95 {milliseconds(summary.p95Ms)}
        </span>
      </output>
      <div data-region="actions" className="flex items-center gap-2">
        <Link to="/agents/$agentId/test" params={{ agentId }} {...buttonLook('secondary')}>
          <ListChecks aria-hidden className="size-4" />
          Run tests
        </Link>
        <Button disabled aria-describedby={laterId}>
          <Rocket aria-hidden className="size-4" />
          Publish
        </Button>
        <span id={laterId} className="rounded-full border border-line px-2 text-sm text-muted">
          Later
        </span>
      </div>
    </div>
  );
}
