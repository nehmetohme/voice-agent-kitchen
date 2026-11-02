import { Link } from '@tanstack/react-router';

import { purposeLabel } from '../state/choices';
import type { AgentListItem } from '../state/models';

/** Agents as rows: the name opens the agent, then its purpose and setup progress. */
export function AgentList({ agents }: { agents: readonly AgentListItem[] }) {
  return (
    <ul aria-label="Agents" className="flex flex-col gap-2">
      {agents.map((agent) => (
        <li
          key={agent.id}
          className="flex items-center justify-between gap-4 rounded-md border border-line bg-surface px-4 py-2"
        >
          <div className="flex min-w-0 flex-col">
            <Link
              to="/agents/$agentId/provision"
              params={{ agentId: agent.id }}
              className="inline-flex min-h-target items-center truncate font-medium underline-offset-4 hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
            >
              {agent.name}
            </Link>
            <span className="text-sm text-muted">{purposeLabel(agent.useCase)}</span>
          </div>
          <span className="shrink-0 text-sm text-muted">{agent.progressPercent} % set up</span>
        </li>
      ))}
    </ul>
  );
}
