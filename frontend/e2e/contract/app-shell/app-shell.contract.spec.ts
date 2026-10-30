// Contract: app shell v1. Pinned by the signed tag contract/app-shell/v1.
// Never edit this file; to change the contract, add a v2 and get it signed.
import { expect, test, type Locator, type Page } from '@playwright/test';

import { auditScreen } from '../../support/ux-audit';

const SCREENS = [
  { path: '/agents', title: 'Agents' },
  { path: '/settings', title: 'Settings' },
] as const;

function primaryNav(page: Page): Locator {
  return page.getByRole('navigation', { name: 'Primary' });
}

async function boxOf(
  locator: Locator,
): Promise<{ x: number; y: number; width: number; height: number }> {
  const box = await locator.boundingBox();
  if (box === null) throw new Error(`${String(locator)} has no layout box`);
  return box;
}

test.describe('app shell v1', { tag: '@contract' }, () => {
  test('opens on Agents', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveURL(/\/agents$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Agents' })).toBeVisible();
  });

  for (const screen of SCREENS) {
    test.describe(screen.path, () => {
      test.beforeEach(async ({ page }) => {
        await page.goto(screen.path);
        await expect(page.getByRole('heading', { level: 1, name: screen.title })).toBeVisible();
      });

      test('has a left sidebar holding the primary navigation', async ({ page }) => {
        const viewport = page.viewportSize();
        if (viewport === null) throw new Error('the test needs a fixed viewport');
        const nav = await boxOf(primaryNav(page));
        const main = await boxOf(page.getByRole('main'));
        expect(nav.x + nav.width).toBeLessThanOrEqual(viewport.width / 3);
        expect(main.x).toBeGreaterThanOrEqual(nav.x + nav.width);
      });

      test('lists at most 7 primary links, Agents first and Settings last', async ({ page }) => {
        const links = primaryNav(page).getByRole('link');
        const count = await links.count();
        expect(count).toBeGreaterThanOrEqual(2);
        expect(count).toBeLessThanOrEqual(7);
        await expect(links.first()).toHaveAccessibleName('Agents');
        await expect(links.last()).toHaveAccessibleName('Settings');
      });

      test('has a top bar above the main content', async ({ page }) => {
        const banner = page.getByRole('banner');
        await expect(banner).toBeVisible();
        const top = await boxOf(banner);
        const main = await boxOf(page.getByRole('main'));
        expect(top.y + top.height).toBeLessThanOrEqual(main.y);
      });

      test('shows at most one primary call to action', async ({ page }) => {
        expect(await page.locator('[data-variant="primary"]:visible').count()).toBeLessThanOrEqual(
          1,
        );
      });

      test('passes the UX audit', async ({ page }) => {
        await auditScreen(page);
      });
    });
  }

  test('marks the current section and moves between sections', async ({ page }) => {
    await page.goto('/agents');
    const agents = primaryNav(page).getByRole('link', { name: 'Agents', exact: true });
    const settings = primaryNav(page).getByRole('link', { name: 'Settings', exact: true });
    await expect(agents).toHaveAttribute('aria-current', 'page');

    await settings.click();
    await expect(page).toHaveURL(/\/settings$/);
    await expect(page.getByRole('heading', { level: 1, name: 'Settings' })).toBeVisible();
    await expect(settings).toHaveAttribute('aria-current', 'page');
    await expect(agents).not.toHaveAttribute('aria-current', 'page');
  });
});
