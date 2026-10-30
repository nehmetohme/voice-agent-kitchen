import { describe, expect, it } from 'vitest';

import { e2ePorts, slotFor } from './ports';

describe('e2ePorts', () => {
  it('derives the same ports for the same worktree', () => {
    expect(e2ePorts('/repo/.claude/worktrees/a', {})).toEqual(
      e2ePorts('/repo/.claude/worktrees/a', {}),
    );
  });

  it('keeps every derived port in its range, on the same slot', () => {
    for (const worktree of ['/a', '/b', '/repo/.claude/worktrees/task-1', '/x/y/z']) {
      const slot = slotFor(worktree);
      expect(slot).toBeGreaterThanOrEqual(0);
      expect(slot).toBeLessThan(800);
      expect(e2ePorts(worktree, {})).toEqual({
        web: 4200 + slot,
        api: 14_200 + slot,
        db: 24_200 + slot,
      });
    }
  });

  it('gives different worktrees different ports', () => {
    const worktrees = Array.from({ length: 20 }, (_, index) => `/repo/.claude/worktrees/${index}`);
    const ports = new Set(worktrees.map((worktree) => e2ePorts(worktree, {}).web));
    expect(ports.size).toBeGreaterThan(15);
  });

  it('lets each variable override its port', () => {
    const ports = e2ePorts('/a', { E2E_PORT: '4175', E2E_API_PORT: '3001', E2E_DB_PORT: ' ' });
    expect(ports.web).toBe(4175);
    expect(ports.api).toBe(3001);
    expect(ports.db).toBe(24_200 + slotFor('/a'));
  });

  it('rejects a port that is not a whole number in range', () => {
    expect(() => e2ePorts('/a', { E2E_PORT: 'abc' })).toThrow(/E2E_PORT/);
    expect(() => e2ePorts('/a', { E2E_API_PORT: '80' })).toThrow(/E2E_API_PORT/);
    expect(() => e2ePorts('/a', { E2E_DB_PORT: '70000' })).toThrow(/E2E_DB_PORT/);
  });
});
