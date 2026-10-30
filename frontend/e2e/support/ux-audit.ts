/**
 * auditScreen(page): the automated UX-law gate. Every E2E screen calls it once it has settled.
 * The rules and their reasons live in docs/ux-guardrails/; the limits below are the measurable
 * parts. A failure lists every finding with the law it breaks.
 */
import { AxeBuilder } from '@axe-core/playwright';
import { expect, type Page } from '@playwright/test';

const UX_LIMITS = {
  /** Von Restorff, Hick: at most one visible primary action. */
  maxPrimaryActions: 1,
  /** Miller: primary navigation stays at 7 items or fewer. */
  maxPrimaryNavLinks: 7,
  /** Hick: 1 primary + 3 secondary actions per `[data-region="actions"]`. */
  maxActionsPerRegion: 4,
  /** Miller: at most 7 inputs per fieldset. */
  maxInputsPerFieldset: 7,
  /** Fitts (WCAG 2.5.5 enhanced): interactive targets are at least 44 × 44 CSS px. */
  minTargetPx: 44,
} as const;

/** axe-core tags for WCAG 2.2 level AA. */
const WCAG_22_AA = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

export interface UxFinding {
  law: string;
  message: string;
  elements?: string[];
}

const TARGETS = [
  'button',
  '[role="button"]',
  '[role="tab"]',
  '[role="checkbox"]',
  '[role="radio"]',
  '[role="switch"]',
  '[role="menuitem"]',
  'input[type="checkbox"]',
  'input[type="radio"]',
  'nav a[href]',
].join(',');

const FORM_CONTROLS = 'input:not([type="hidden"]),select,textarea,[role="combobox"]';

async function visibleCount(page: Page, selector: string): Promise<number> {
  return page.locator(`${selector} >> visible=true`).count();
}

async function smallTargets(page: Page, minPx: number): Promise<string[]> {
  return page.locator(TARGETS).evaluateAll((elements, min) => {
    const small: string[] = [];
    for (const element of elements) {
      if (!element.checkVisibility()) continue;
      // A native checkbox or radio is hit through its label too.
      const labels = element instanceof HTMLInputElement ? [...(element.labels ?? [])] : [];
      const rects = [element, ...labels].map((node) => node.getBoundingClientRect());
      const width = Math.max(...rects.map((rect) => rect.width));
      const height = Math.max(...rects.map((rect) => rect.height));
      if (width < min || height < min) {
        small.push(`${Math.round(width)}×${Math.round(height)} ${element.outerHTML.slice(0, 80)}`);
      }
    }
    return small;
  }, minPx);
}

async function crowdedRegions(page: Page, max: number): Promise<string[]> {
  return page.locator('[data-region="actions"]').evaluateAll((regions, limit) => {
    return regions
      .map((region) => {
        const actions = [...region.querySelectorAll('button,[role="button"],a[href]')];
        return { region, count: actions.filter((action) => action.checkVisibility()).length };
      })
      .filter(({ count }) => count > limit)
      .map(({ region, count }) => `${count} actions in ${region.outerHTML.slice(0, 60)}`);
  }, max);
}

async function crowdedFieldsets(page: Page, max: number): Promise<string[]> {
  return page.locator('fieldset').evaluateAll(
    (fieldsets, { limit, controls }) =>
      fieldsets
        .map((fieldset) => ({ fieldset, count: fieldset.querySelectorAll(controls).length }))
        .filter(({ count }) => count > limit)
        .map(({ fieldset, count }) => `${count} inputs in ${fieldset.outerHTML.slice(0, 60)}`),
    { limit: max, controls: FORM_CONTROLS },
  );
}

/** Uniform connectedness: every radio, and every checkbox set, sits in a named group. */
async function ungroupedChoices(page: Page): Promise<string[]> {
  return page
    .locator('input[type="radio"],[role="radio"],input[type="checkbox"]')
    .evaluateAll((choices) => {
      const named =
        'fieldset:has(> legend),[role="radiogroup"][aria-label],[role="radiogroup"][aria-labelledby],' +
        '[role="group"][aria-label],[role="group"][aria-labelledby]';
      return choices
        .filter((choice) => {
          if (!choice.checkVisibility()) return false;
          const name = choice.getAttribute('name');
          const isCheckbox = choice.getAttribute('type') === 'checkbox';
          const siblings = name
            ? document.querySelectorAll(`[name="${CSS.escape(name)}"]`).length
            : 1;
          if (isCheckbox && siblings < 2) return false;
          return choice.closest(named) === null;
        })
        .map((choice) => choice.outerHTML.slice(0, 80));
    });
}

/** Runs every check and returns the findings; `auditScreen` asserts there are none. */
export async function collectUxFindings(page: Page): Promise<UxFinding[]> {
  const findings: UxFinding[] = [];
  const primaries = await visibleCount(page, '[data-variant="primary"]');
  if (primaries > UX_LIMITS.maxPrimaryActions) {
    findings.push({
      law: 'Von Restorff / Hick',
      message: `${primaries} visible primary actions (max 1)`,
    });
  }
  const navLinks = await page
    .getByRole('navigation', { name: 'Primary' })
    .getByRole('link')
    .count();
  if (navLinks > UX_LIMITS.maxPrimaryNavLinks) {
    findings.push({ law: 'Miller', message: `${navLinks} primary navigation links (max 7)` });
  }
  const checks: [string, string, string[]][] = [
    ['Fitts', 'targets smaller than 44×44 CSS px', await smallTargets(page, UX_LIMITS.minTargetPx)],
    [
      'Hick',
      'action regions with more than 4 actions',
      await crowdedRegions(page, UX_LIMITS.maxActionsPerRegion),
    ],
    [
      'Miller',
      'fieldsets with more than 7 inputs',
      await crowdedFieldsets(page, UX_LIMITS.maxInputsPerFieldset),
    ],
    ['Uniform connectedness', 'choices outside a named group', await ungroupedChoices(page)],
  ];
  for (const [law, message, elements] of checks) {
    if (elements.length > 0) findings.push({ law, message, elements });
  }
  const axe = await new AxeBuilder({ page }).withTags(WCAG_22_AA).analyze();
  for (const violation of axe.violations) {
    findings.push({
      law: 'Accessibility (WCAG 2.2 AA)',
      message: `${violation.id}: ${violation.help}`,
      elements: violation.nodes.map((node) => node.html.slice(0, 80)),
    });
  }
  return findings;
}

/** Fails the test with every UX-law finding on the current screen. */
export async function auditScreen(page: Page): Promise<void> {
  const findings = await collectUxFindings(page);
  const report = findings
    .map(
      (finding) =>
        `- [${finding.law}] ${finding.message}\n    ${(finding.elements ?? []).join('\n    ')}`,
    )
    .join('\n');
  expect(findings, `UX audit failed on ${page.url()}:\n${report}`).toEqual([]);
}
