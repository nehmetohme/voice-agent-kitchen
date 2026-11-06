import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import type { ComponentType } from 'react';

import { EmptyState } from '../../../shared/ui/empty-state';
import { LoadFailed, Loading } from '../../../shared/ui/load-states';
import { isNotFound, type PurposeDetails } from '../state/models';
import { purposeQueryOptions, toolCatalogQueryOptions } from '../state/purposes-queries';
import { sectionLabel, type PurposeSection } from '../state/sections';
import { usePurposeDraft } from '../state/use-purpose-draft';
import { ComplianceSection } from './compliance-section';
import { GuardrailsSection } from './guardrails-section';
import { OverviewSection } from './overview-section';
import { PurposeHeader } from './purpose-header';
import { PurposeSections } from './purpose-sections';
import { ToolsSection } from './tools-section';

interface PurposePageProps {
  purposeId: string;
  section: PurposeSection;
  /** Composed in by the route from the plans feature: the flow editor, for the Flow section. */
  flow: ComponentType<{ purposeId: string; readOnly: boolean }>;
}

/** The server's message for the overview's fields, after a refused save. */
function overviewErrors(errors: readonly { field: string; message: string }[]) {
  const named = (field: string) => errors.find((error) => error.field === field)?.message;
  return {
    name: named('name'),
    description: named('description'),
    complianceClass: named('complianceClass'),
    persona: named('persona'),
  };
}

function Workspace({
  purpose,
  section,
  flow: Flow,
}: Omit<PurposePageProps, 'purposeId'> & {
  purpose: PurposeDetails;
}) {
  const draft = usePurposeDraft(purpose);
  const catalog = useQuery(toolCatalogQueryOptions());
  const { draft: settings, edit } = draft;
  const readOnly = purpose.system;
  return (
    <div className="flex flex-col gap-4 lg:h-full">
      <PurposeHeader purpose={purpose} draft={draft} />
      <PurposeSections purposeId={purpose.id} current={section} />
      <section
        aria-label={sectionLabel(section)}
        // A keyboard user scrolls the section from here (WCAG 2.1.1; axe scrollable-region).
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        className="flex flex-col gap-6 pb-4 focus-visible:outline-2 focus-visible:outline-focus lg:min-h-0 lg:flex-1 lg:overflow-y-auto"
      >
        {readOnly && (
          <p className="text-sm text-muted">
            Built-in purposes are read-only. Duplicate this one to change it.
          </p>
        )}
        {section === 'overview' && (
          <OverviewSection
            overview={settings.overview}
            onChange={(overview) => {
              edit('overview', overview);
            }}
            readOnly={readOnly}
            errors={overviewErrors(draft.errors)}
          />
        )}
        {section === 'flow' && <Flow purposeId={purpose.id} readOnly={readOnly} />}
        {section === 'tools' && (
          <ToolsSection
            tools={settings.tools}
            onChange={(tools) => {
              edit('tools', tools);
            }}
            catalog={catalog.data}
            readOnly={readOnly}
            problems={purpose.problems}
          />
        )}
        {section === 'guardrails' && (
          <GuardrailsSection
            guardrails={settings.guardrails}
            onChange={(guardrails) => {
              edit('guardrails', guardrails);
            }}
            packClaims={purpose.baselines[0]?.forbiddenClaims ?? []}
            readOnly={readOnly}
          />
        )}
        {section === 'compliance' && (
          <ComplianceSection
            compliance={settings.compliance}
            onChange={(compliance) => {
              edit('compliance', compliance);
            }}
            baselines={purpose.baselines}
            readOnly={readOnly}
          />
        )}
      </section>
    </div>
  );
}

/**
 * One purpose as a workspace that fits the screen, like the agent page: the header with its
 * version state and actions, the section links, then the section, which scrolls on its own
 * from `lg`. Every edit autosaves; "Save version N" freezes the draft.
 */
export function PurposePage({ purposeId, section, flow }: PurposePageProps) {
  const purpose = useQuery(purposeQueryOptions(purposeId));
  if (purpose.isPending) return <Loading label="Loading the purpose…" />;
  if (purpose.isError) {
    if (isNotFound(purpose.error)) {
      return (
        <EmptyState
          title="This purpose doesn't exist"
          description="The address may be wrong."
          action={
            <Link to="/purposes" className="font-medium underline underline-offset-4">
              Go to Purposes
            </Link>
          }
        />
      );
    }
    return (
      <LoadFailed
        what="this purpose"
        onRetry={() => {
          void purpose.refetch();
        }}
      />
    );
  }
  return <Workspace key={purposeId} purpose={purpose.data} section={section} flow={flow} />;
}
