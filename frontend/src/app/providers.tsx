import { QueryClientProvider, type QueryClient } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';

import type { AppRouter } from './router';

interface AppProvidersProps {
  queryClient: QueryClient;
  router: AppRouter;
}

/** Everything the app runs inside: server-state cache, then the router. */
export function AppProviders({ queryClient, router }: AppProvidersProps) {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
