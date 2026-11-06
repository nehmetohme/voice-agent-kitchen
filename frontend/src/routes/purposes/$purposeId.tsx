import { createFileRoute } from '@tanstack/react-router';
import { z } from 'zod';

import { PurposeFlow } from '../../features/plans';
import { PurposePage, SECTION_VALUES, useRefreshPurpose } from '../../features/purposes';

/** `?section=`: an unknown or missing section opens the overview (Postel). */
const search = z.object({ section: z.enum(SECTION_VALUES).default('overview').catch('overview') });

export const Route = createFileRoute('/purposes/$purposeId')({
  staticData: { title: 'Purpose' },
  validateSearch: search,
  component: PurposeRoute,
});

/** The plans feature's flow editor on the purpose's starting flow; a save refreshes the purpose. */
function Flow({ purposeId, readOnly }: { purposeId: string; readOnly: boolean }) {
  const refresh = useRefreshPurpose(purposeId);
  return <PurposeFlow purposeId={purposeId} readOnly={readOnly} onSaved={refresh} />;
}

/** The purpose's page, with the flow editor composed in for its Flow section. */
function PurposeRoute() {
  const { purposeId } = Route.useParams();
  const { section } = Route.useSearch();
  return <PurposePage purposeId={purposeId} section={section} flow={Flow} />;
}
