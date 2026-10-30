import { createFileRoute, redirect } from '@tanstack/react-router';

// Agents is home: the first item of the primary navigation.
export const Route = createFileRoute('/')({
  beforeLoad: () => redirect({ to: '/agents', throw: true }),
});
