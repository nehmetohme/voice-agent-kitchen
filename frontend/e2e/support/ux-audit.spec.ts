// Gate self-test: proves auditScreen() fails when it should. Uses static HTML, not the app.
import { expect, test, type Page } from '@playwright/test';

import { auditScreen, collectUxFindings } from './ux-audit';

const TARGET = 'style="display:inline-block;min-width:44px;min-height:44px"';

async function render(page: Page, body: string): Promise<void> {
  await page.setContent(
    `<!doctype html><html lang="en"><head><title>Audit fixture</title></head><body>${body}</body></html>`,
  );
}

async function laws(page: Page): Promise<string[]> {
  return (await collectUxFindings(page)).map((finding) => finding.law);
}

test.describe('auditScreen', () => {
  test('passes a compliant screen', async ({ page }) => {
    await render(
      page,
      `<nav aria-label="Primary"><a href="/agents" ${TARGET}>Agents</a></nav>
       <main><h1>Agents</h1><div data-region="actions">
         <button data-variant="primary" ${TARGET}>Create agent</button>
         <button data-variant="secondary" ${TARGET}>Import</button>
       </div></main>`,
    );
    await auditScreen(page);
  });

  test('fails on a second primary action (Von Restorff)', async ({ page }) => {
    await render(
      page,
      `<main><button data-variant="primary" ${TARGET}>Save</button>
       <button data-variant="primary" ${TARGET}>Publish</button></main>`,
    );
    expect(await laws(page)).toContain('Von Restorff / Hick');
    await expect(auditScreen(page)).rejects.toThrow(/2 visible primary actions/);
  });

  test('fails on more than 7 primary navigation links (Miller)', async ({ page }) => {
    const links = Array.from({ length: 8 }, (_, i) => `<a href="/${i}" ${TARGET}>Item ${i}</a>`);
    await render(page, `<nav aria-label="Primary">${links.join('')}</nav>`);
    expect(await laws(page)).toContain('Miller');
  });

  test('fails on a target smaller than 44 × 44 px (Fitts)', async ({ page }) => {
    await render(page, '<main><button style="width:32px;height:32px">Go</button></main>');
    expect(await laws(page)).toContain('Fitts');
  });

  test('fails on more than 4 actions in one region (Hick)', async ({ page }) => {
    const buttons = Array.from({ length: 5 }, (_, i) => `<button ${TARGET}>Action ${i}</button>`);
    await render(page, `<main><div data-region="actions">${buttons.join('')}</div></main>`);
    expect(await laws(page)).toContain('Hick');
  });

  test('fails on radios outside a named group (uniform connectedness)', async ({ page }) => {
    await render(
      page,
      `<main><label>A <input type="radio" name="voice" ${TARGET}></label>
       <label>B <input type="radio" name="voice" ${TARGET}></label></main>`,
    );
    expect(await laws(page)).toContain('Uniform connectedness');
  });

  test('fails on an axe WCAG 2.2 AA violation', async ({ page }) => {
    await render(page, `<main><button ${TARGET}></button></main>`);
    expect(await laws(page)).toContain('Accessibility (WCAG 2.2 AA)');
  });
});
