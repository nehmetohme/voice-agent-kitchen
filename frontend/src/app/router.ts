import type { QueryClient } from '@tanstack/react-query';
import { createRouter, type RouterHistory } from '@tanstack/react-router';

import { NotFound, RouteError } from './error-boundary';
import { routeTree } from './routeTree.gen';

interface AppRouterOptions {
  queryClient: QueryClient;
  /** Tests pass a memory history; the app uses the browser history. */
  history?: RouterHistory;
}

export function createAppRouter({ queryClient, history }: AppRouterOptions) {
  return createRouter({
    routeTree,
    context: { queryClient },
    ...(history === undefined ? {} : { history }),
    defaultPreload: 'intent',
    // Let TanStack Query decide freshness; the router always asks it.
    defaultPreloadStaleTime: 0,
    defaultErrorComponent: RouteError,
    defaultNotFoundComponent: NotFound,
    scrollRestoration: true,
  });
}

export type AppRouter = ReturnType<typeof createAppRouter>;

declare module '@tanstack/react-router' {
  interface Register {
    router: AppRouter;
  }
}
