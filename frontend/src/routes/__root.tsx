import type { QueryClient } from '@tanstack/react-query';
import { createRootRouteWithContext, Link, Outlet, useMatches } from '@tanstack/react-router';
import { AudioLines, Bot, Settings } from 'lucide-react';
import { useEffect } from 'react';

import { AppShell, SidebarNav, SidebarNavItem, sidebarLinkClassName } from '../shared/ui/app-shell';

declare module '@tanstack/react-router' {
  interface StaticDataRouteOption {
    /** Page title, shown in the top bar and the browser tab. */
    title?: string;
  }
}

/** What every route's loader can use. */
export interface RouterContext {
  queryClient: QueryClient;
}

const PRODUCT = 'Voice Agent Kitchen';

// Serial position: Agents first, Settings last. Miller: 7 items at most.
const PRIMARY_NAV = [
  { to: '/agents', label: 'Agents', Icon: Bot },
  { to: '/settings', label: 'Settings', Icon: Settings },
] as const;

export const Route = createRootRouteWithContext<RouterContext>()({ component: RootLayout });

function usePageTitle(): string {
  const title = useMatches({
    select: (matches) => matches.findLast((match) => match.staticData.title)?.staticData.title,
  });
  useEffect(() => {
    document.title = title === undefined ? PRODUCT : `${title} · ${PRODUCT}`;
  }, [title]);
  return title ?? PRODUCT;
}

function RootLayout() {
  const title = usePageTitle();
  return (
    <AppShell
      brand={
        <p className="flex min-h-target items-center gap-2 px-3 font-semibold">
          <AudioLines aria-hidden className="size-6 text-muted" />
          {PRODUCT}
        </p>
      }
      navigation={
        <SidebarNav label="Primary">
          {PRIMARY_NAV.map(({ to, label, Icon }) => (
            <SidebarNavItem key={to}>
              <Link to={to} className={sidebarLinkClassName}>
                <Icon aria-hidden className="size-4" />
                {label}
              </Link>
            </SidebarNavItem>
          ))}
        </SidebarNav>
      }
      topBar={<h1 className="text-xl font-semibold">{title}</h1>}
    >
      <Outlet />
    </AppShell>
  );
}
