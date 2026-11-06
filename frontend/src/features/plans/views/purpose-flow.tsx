import { useQuery } from '@tanstack/react-query';

import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { flowQueryOptions } from '../state/plans-queries';
import { PlanEditor } from './plan-editor';

interface PurposeFlowProps {
  purposeId: string;
  /** A built-in purpose's flow is shown, not edited. */
  readOnly: boolean;
  /** Called after each save, e.g. to refresh the purpose's problems and version state. */
  onSaved?: (() => void) | undefined;
}

/** A purpose's starting flow in the flow editor: what every agent made from it starts with. */
export function PurposeFlow({ purposeId, readOnly, onSaved }: PurposeFlowProps) {
  const target = { kind: 'purpose', purposeId } as const;
  const flow = useQuery(flowQueryOptions(target));
  if (flow.isPending) return <Loading label="Loading the flow…" />;
  if (flow.isError) {
    return (
      <LoadFailed
        what="the flow"
        onRetry={() => {
          void flow.refetch();
        }}
      />
    );
  }
  return (
    <PlanEditor
      key={purposeId}
      target={target}
      initial={flow.data}
      readOnly={readOnly}
      onSaved={onSaved}
    />
  );
}
