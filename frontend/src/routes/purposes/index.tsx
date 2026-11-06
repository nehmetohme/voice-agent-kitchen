import { createFileRoute } from '@tanstack/react-router';

import { PurposesOverview } from '../../features/purposes';

export const Route = createFileRoute('/purposes/')({
  staticData: { title: 'Purposes' },
  component: PurposesOverview,
});
