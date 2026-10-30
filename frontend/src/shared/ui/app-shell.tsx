import type { ReactNode } from 'react';

import { cn } from '../lib/cn';

interface AppShellProps {
  /** Product name or logo at the top of the sidebar. */
  brand: ReactNode;
  /** The primary navigation, usually a <SidebarNav label="Primary">. */
  navigation: ReactNode;
  /** Content of the top bar: the page title and, later, page-level tools. */
  topBar: ReactNode;
  children: ReactNode;
}

/** The standard app frame users know (Jakob): left sidebar, top bar, main content. */
export function AppShell({ brand, navigation, topBar, children }: AppShellProps) {
  return (
    <div className="flex min-h-screen bg-canvas text-ink">
      <aside
        aria-label="Sidebar"
        className="flex w-sidebar shrink-0 flex-col gap-6 border-r border-line bg-surface p-4"
      >
        {brand}
        {navigation}
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex min-h-16 items-center border-b border-line px-6">{topBar}</header>
        <main className="flex-1 px-6 py-8">{children}</main>
      </div>
    </div>
  );
}

/** A labelled list of navigation links; keep it to 7 items or fewer (Miller). */
export function SidebarNav({ label, children }: { label: string; children: ReactNode }) {
  return (
    <nav aria-label={label}>
      <ul className="flex flex-col gap-1">{children}</ul>
    </nav>
  );
}

export function SidebarNavItem({ children }: { children: ReactNode }) {
  return <li>{children}</li>;
}

/**
 * Classes for a link inside <SidebarNavItem>. Routers render their own link element, so this
 * is a class list rather than a component. The current page is styled from `aria-current`.
 */
export const sidebarLinkClassName = cn(
  'flex min-h-target items-center gap-3 rounded-md px-3 text-sm font-medium text-muted',
  'transition-colors hover:bg-hover hover:text-ink',
  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus',
  'aria-[current=page]:bg-selected aria-[current=page]:text-ink',
);
