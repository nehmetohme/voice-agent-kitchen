import { CircleAlert } from 'lucide-react';

import { ConfirmDialog } from '../../../shared/ui/confirm-dialog';
import type { HttpToolDraft, PurposeProblem } from '../state/models';
import { HttpToolDialog } from './http-tool-dialog';

interface HttpToolCardProps {
  tool: HttpToolDraft;
  /** The draft's problem with this tool (its host off the allow-list), if any. */
  problem: PurposeProblem | undefined;
  readOnly: boolean;
  /** The purpose's other tool names and the built-ins' names: a new name must differ. */
  others: readonly string[];
  builtins: readonly string[];
  allowedHosts: readonly string[];
  onSave: (tool: HttpToolDraft) => void;
  onRemove: () => void;
}

/** One custom tool as a named group: where it calls, what it needs, and its own actions. */
export function HttpToolCard({
  tool,
  problem,
  readOnly,
  others,
  builtins,
  allowedHosts,
  onSave,
  onRemove,
}: HttpToolCardProps) {
  const inputs = tool.inputs.map((input) => input.name).join(', ');
  return (
    <fieldset
      aria-label={tool.name}
      className="flex flex-col gap-2 rounded-md border border-line p-4"
    >
      <p className="font-mono text-sm font-semibold">{tool.name}</p>
      <p className="text-sm">{tool.description}</p>
      <p className="text-sm break-all text-muted">{`${tool.method} ${tool.url}`}</p>
      <p className="text-sm text-muted">
        {inputs === '' ? 'No inputs' : `Inputs: ${inputs}`}
        {tool.secret === '' ? '' : ` · Secret: ${tool.secret}`}
        {tool.confirm ? ' · Confirms first' : ''}
      </p>
      {problem !== undefined && (
        <p className="flex gap-2 text-sm text-danger">
          <CircleAlert aria-hidden className="mt-1 size-4 shrink-0" />
          {problem.message}
        </p>
      )}
      {!readOnly && (
        <div data-region="actions" className="flex gap-2">
          <HttpToolDialog
            tool={tool}
            taken={others}
            builtins={builtins}
            allowedHosts={allowedHosts}
            onSave={onSave}
          />
          <ConfirmDialog
            triggerLabel="Remove"
            title={`Remove ${tool.name}?`}
            description="Agents on a saved version keep it until they update."
            confirmLabel="Remove"
            onConfirm={onRemove}
          />
        </div>
      )}
    </fieldset>
  );
}
