import { createFileRoute } from '@tanstack/react-router';
import { Settings } from 'lucide-react';

import { EmptyState } from '../shared/ui/empty-state';

export const Route = createFileRoute('/settings')({
  staticData: { title: 'Settings' },
  component: SettingsPage,
});

function SettingsPage() {
  return (
    <EmptyState
      icon={<Settings aria-hidden className="size-8 text-muted" />}
      title="Nothing to set up yet"
      description="Workspace settings, such as model profiles and channels, appear here."
    />
  );
}
