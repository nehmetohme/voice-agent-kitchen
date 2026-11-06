import { Link } from '@tanstack/react-router';

import { SectionNav, sectionLinkClassName } from '../../../shared/ui/section-nav';
import { SECTIONS, type PurposeSection } from '../state/sections';

/** The purpose's sections as links: each has its own address (`?section=`), so back works. */
export function PurposeSections({
  purposeId,
  current,
}: {
  purposeId: string;
  current: PurposeSection;
}) {
  return (
    <SectionNav label="Purpose sections">
      {SECTIONS.map(({ value, label }) => (
        <li key={value}>
          <Link
            to="/purposes/$purposeId"
            params={{ purposeId }}
            search={{ section: value }}
            aria-current={value === current ? 'page' : undefined}
            className={sectionLinkClassName}
          >
            {label}
          </Link>
        </li>
      ))}
    </SectionNav>
  );
}
