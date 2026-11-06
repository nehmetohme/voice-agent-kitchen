import { Lock } from 'lucide-react';

import { CheckboxField } from '../../../shared/ui/checkbox-field';
import type { PurposeProblem, ToolCatalog, ToolSettings } from '../state/models';
import { putHttpTool, removeHttpTool, toggleBuiltin } from '../state/purpose-edits';
import { HttpToolCard } from './http-tool-card';
import { HttpToolDialog } from './http-tool-dialog';

interface ToolsSectionProps {
  tools: ToolSettings;
  onChange: (tools: ToolSettings) => void;
  catalog: ToolCatalog | undefined;
  readOnly: boolean;
  /** The draft's problems about tools (a host off the allow-list). */
  problems: readonly PurposeProblem[];
}

function hostsText(hosts: readonly string[]): string {
  return hosts.length === 0
    ? 'No host is allowed yet: your admin sets the tool host allow-list (VAK_TOOL_HOSTS).'
    : `They may call ${hosts.join(', ')}, set by your admin. Nothing is called until tools run in calls.`;
}

/**
 * The tools a purpose turns on: the platform's built-in ones (the opt-out tool locked on), and
 * custom HTTP tools on the workspace's allowed hosts. Definitions only: tools run with M4b.
 */
export function ToolsSection({ tools, onChange, catalog, readOnly, problems }: ToolsSectionProps) {
  const builtins = catalog?.builtins ?? [];
  const builtinNames = builtins.map((tool) => tool.name);
  const names = tools.http.map((tool) => tool.name);
  const hosts = catalog?.allowedHosts ?? [];
  return (
    <div className="flex max-w-readable flex-col gap-6">
      <fieldset disabled={readOnly} className="flex flex-col gap-1">
        <legend className="text-sm font-medium">Built-in tools</legend>
        {builtins.map((tool) => (
          <CheckboxField
            key={tool.name}
            label={tool.locked ? `${tool.label} (always on)` : tool.label}
            description={tool.description}
            checked={tool.locked || tools.builtins.includes(tool.name)}
            disabled={tool.locked}
            onCheckedChange={(on) => {
              onChange(toggleBuiltin(tools, tool.name, on, tool.locked));
            }}
          />
        ))}
      </fieldset>
      <section aria-label="Custom HTTP tools" className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <h3 className="text-sm font-medium">Custom HTTP tools</h3>
          {!readOnly && (
            <HttpToolDialog
              taken={names}
              builtins={builtinNames}
              allowedHosts={hosts}
              onSave={(tool) => {
                onChange(putHttpTool(tools, tool, null));
              }}
            />
          )}
        </div>
        <p className="flex gap-2 text-sm text-muted">
          <Lock aria-hidden className="mt-1 size-4 shrink-0" />
          {hostsText(hosts)}
        </p>
        {tools.http.length === 0 && <p className="text-sm text-muted">No custom tools yet.</p>}
        {tools.http.map((tool, at) => (
          <HttpToolCard
            key={tool.name}
            tool={tool}
            problem={problems.find((problem) => problem.field === `tools.http[${String(at)}].url`)}
            readOnly={readOnly}
            others={names.filter((name) => name !== tool.name)}
            builtins={builtinNames}
            allowedHosts={hosts}
            onSave={(changed) => {
              onChange(putHttpTool(tools, changed, tool.name));
            }}
            onRemove={() => {
              onChange(removeHttpTool(tools, tool.name));
            }}
          />
        ))}
      </section>
    </div>
  );
}
