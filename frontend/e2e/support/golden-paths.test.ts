// GOLD-1 (docs/ux-guardrails/golden-paths.md): every `active` golden path has a tagged spec.
import { existsSync, readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

const DOC = new URL('../../../docs/ux-guardrails/golden-paths.md', import.meta.url);
const GOLDEN_DIR = new URL('../golden/', import.meta.url);
const STATUSES = new Set(['planned', 'active']);

interface GoldenPath {
  id: string;
  status: string;
}

/** Rows of the paths table: the first cell is a backticked ID, the last is the status. */
function parseGoldenPaths(markdown: string): GoldenPath[] {
  return markdown
    .split('\n')
    .filter((line) => /^\|\s*`[a-z0-9-]+`\s*\|/.test(line))
    .map((line) => {
      const cells = line
        .split('|')
        .slice(1, -1)
        .map((cell) => cell.trim());
      return { id: (cells[0] ?? '').replaceAll('`', ''), status: cells.at(-1) ?? '' };
    });
}

describe('golden paths', () => {
  const paths = parseGoldenPaths(readFileSync(DOC, 'utf8'));

  it('lists paths with a known status', () => {
    expect(paths.length).toBeGreaterThan(0);
    expect(paths.filter((path) => !STATUSES.has(path.status))).toEqual([]);
  });

  it('has an @golden spec for every active path', () => {
    const missing = paths
      .filter((path) => path.status === 'active')
      .filter((path) => {
        const spec = new URL(`${path.id}.golden.spec.ts`, GOLDEN_DIR);
        return !existsSync(spec) || !readFileSync(spec, 'utf8').includes('@golden');
      })
      .map((path) => `e2e/golden/${path.id}.golden.spec.ts`);
    expect(missing).toEqual([]);
  });
});
