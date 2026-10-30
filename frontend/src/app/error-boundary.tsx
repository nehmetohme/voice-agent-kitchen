import { Link, useRouter, type ErrorComponentProps } from '@tanstack/react-router';

import { Button } from '../shared/ui/button';
import { EmptyState } from '../shared/ui/empty-state';

/** Shown when a route throws. Every error state ends with a way forward (peak-end). */
export function RouteError({ reset }: ErrorComponentProps) {
  const router = useRouter();
  const retry = () => {
    reset();
    void router.invalidate();
  };
  return (
    <section
      role="alert"
      className="flex max-w-readable flex-col items-start gap-4 rounded-md border border-line bg-surface p-8"
    >
      <div className="flex flex-col gap-2">
        <h2 className="text-lg font-semibold">Something went wrong</h2>
        <p className="text-muted">This page could not be shown. Try again, or reload the app.</p>
      </div>
      <Button onClick={retry}>Try again</Button>
    </section>
  );
}

/** Shown for a URL that matches no route. */
export function NotFound() {
  return (
    <EmptyState
      title="Page not found"
      description="This address does not match any page."
      action={
        <Link to="/agents" className="font-medium underline underline-offset-4">
          Go to Agents
        </Link>
      }
    />
  );
}
