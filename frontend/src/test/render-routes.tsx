// Renders components at their paths inside a memory router and a fresh QueryClient, so a
// feature's views can be tested with real links and navigation but without the app module.
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router';
import { render } from '@testing-library/react';
import type { ReactNode } from 'react';

export interface TestRoute {
  path: string;
  component: () => ReactNode;
}

export function renderRoutes(path: string, routes: readonly TestRoute[]) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const root = createRootRoute({ component: Outlet });
  const children = routes.map((route) =>
    createRoute({ getParentRoute: () => root, path: route.path, component: route.component }),
  );
  const router = createRouter({
    routeTree: root.addChildren(children),
    history: createMemoryHistory({ initialEntries: [path] }),
  });
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { router, queryClient };
}
