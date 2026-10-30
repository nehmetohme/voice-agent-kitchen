import { readFile } from 'node:fs/promises';

import { describe, expect, it } from 'vitest';

import { OUTPUT, renderThemeCss } from './build.ts';
import tokens from './vak.tokens.json';

/** Number of tokens in a DTCG group (keys starting with `$` are metadata). */
function count(group: object): number {
  return Object.keys(group).filter((key) => !key.startsWith('$')).length;
}

function remToPx(value: string): number {
  const match = /^([\d.]+)(rem|px)$/.exec(value);
  if (!match) throw new Error(`not a rem/px dimension: ${value}`);
  return Number(match[1]) * (match[2] === 'rem' ? 16 : 1);
}

describe('design tokens', () => {
  it('src/shared/ui/tokens.css matches the token files (run `pnpm tokens`)', async () => {
    expect(await readFile(OUTPUT, 'utf8')).toBe(await renderThemeCss());
  });

  it('resets the Tailwind default theme, so off-token classes do not exist', async () => {
    expect(await renderThemeCss()).toContain('--*: initial;');
  });

  // Prägnanz: a small, simple visual vocabulary (docs/ux-guardrails/perception.md).
  it('keeps at most 3 radii, 3 elevations, 2 font families and 6 type sizes', () => {
    expect(count(tokens.radius)).toBeLessThanOrEqual(3);
    expect(count(tokens.shadow)).toBeLessThanOrEqual(3);
    expect(count(tokens.font)).toBeLessThanOrEqual(2);
    expect(count(tokens.text)).toBeLessThanOrEqual(6);
  });

  it('defines no gradients', () => {
    expect(JSON.stringify(tokens)).not.toMatch(/gradient/i);
  });

  // Fitts: interactive targets are at least 44 × 44 CSS px (docs/ux-guardrails/interaction.md).
  it('sizes the minimum target at 44 CSS px or more', () => {
    expect(remToPx(tokens.spacing.target.$value)).toBeGreaterThanOrEqual(44);
  });

  // Proximity: spacing follows the 4-point scale.
  const spacing = Object.entries(tokens.spacing).flatMap(([name, token]) =>
    typeof token === 'object' ? [{ name, px: remToPx(token.$value) }] : [],
  );
  it.each(spacing)('keeps spacing-$name on the 4-point grid', ({ px }) => {
    expect(px % 4).toBe(0);
  });
});
