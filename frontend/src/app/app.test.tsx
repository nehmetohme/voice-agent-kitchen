import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { fakeAgentsApi } from '../test/agents-api';
import { startMswServer } from '../test/msw-server';
import { RouteError } from './error-boundary';
import { AppProviders } from './providers';
import { createQueryClient } from './query-client';
import { createAppRouter } from './router';

// The Agents home lists agents from the API; start every test with none.
startMswServer(...fakeAgentsApi().handlers);

function renderApp(path: string) {
  const queryClient = createQueryClient();
  const history = createMemoryHistory({ initialEntries: [path] });
  const router = createAppRouter({ queryClient, history });
  render(<AppProviders queryClient={queryClient} router={router} />);
  return router;
}

async function heading(name: string) {
  return screen.findByRole('heading', { level: 1, name });
}

describe('app shell', () => {
  it('opens on Agents and shows its empty state', async () => {
    const router = renderApp('/');
    await heading('Agents');
    expect(router.state.location.pathname).toBe('/agents');
    expect(await screen.findByRole('heading', { name: 'No agents yet' })).toBeVisible();
    expect(screen.getByRole('link', { name: 'Create agent' })).toHaveAttribute(
      'href',
      '/agents/new',
    );
    await waitFor(() => {
      expect(document.title).toBe('Agents · Voice Agent Kitchen');
    });
  });

  it('lists Agents first and Settings last in the primary navigation', async () => {
    renderApp('/agents');
    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    const links = within(nav).getAllByRole('link');
    expect(links.map((link) => link.textContent)).toEqual(['Agents', 'Settings']);
    expect(links[0]).toHaveAttribute('aria-current', 'page');
  });

  it('moves to Settings from the sidebar', async () => {
    renderApp('/agents');
    const nav = await screen.findByRole('navigation', { name: 'Primary' });
    fireEvent.click(within(nav).getByRole('link', { name: 'Settings' }));
    await heading('Settings');
    expect(screen.getByRole('heading', { name: 'Nothing to set up yet' })).toBeVisible();
    expect(within(nav).getByRole('link', { name: 'Settings' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('shows a way back for an unknown address', async () => {
    renderApp('/nowhere');
    expect(await screen.findByRole('heading', { name: 'Page not found' })).toBeVisible();
    expect(screen.getByRole('link', { name: 'Go to Agents' })).toHaveAttribute('href', '/agents');
    await heading('Voice Agent Kitchen');
  });
});

describe('RouteError', () => {
  it('reports the failure and retries the route', async () => {
    let fail = true;
    const root = createRootRoute();
    const page = createRoute({
      getParentRoute: () => root,
      path: '/',
      component: () => {
        if (fail) throw new Error('boom');
        return <p>Recovered</p>;
      },
    });
    const router = createRouter({
      routeTree: root.addChildren([page]),
      history: createMemoryHistory({ initialEntries: ['/'] }),
      defaultErrorComponent: RouteError,
    });
    render(<RouterProvider router={router} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Something went wrong');
    fail = false;
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('Recovered')).toBeVisible();
  });
});
