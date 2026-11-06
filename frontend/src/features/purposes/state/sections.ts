/** The sections of a purpose's page, in order; the route validates `?section=` against them. */
export const SECTION_VALUES = ['overview', 'flow', 'tools', 'guardrails', 'compliance'] as const;

export type PurposeSection = (typeof SECTION_VALUES)[number];

const LABELS: Record<PurposeSection, string> = {
  overview: 'Overview',
  flow: 'Flow',
  tools: 'Tools',
  guardrails: 'Guardrails',
  compliance: 'Compliance',
};

export const SECTIONS = SECTION_VALUES.map((value) => ({ value, label: LABELS[value] }));

/** The section's label, e.g. `Tools`. */
export function sectionLabel(section: PurposeSection): string {
  return LABELS[section];
}
