import { describe, expect, it } from 'vitest';

import { cn } from './cn';

describe('cn', () => {
  it('joins truthy class names', () => {
    expect(cn('flex', false, undefined, 'gap-2', { hidden: false, 'p-4': true })).toBe(
      'flex gap-2 p-4',
    );
  });

  it('lets a later Tailwind class override an earlier one', () => {
    expect(cn('p-2 text-ink', 'p-4')).toBe('text-ink p-4');
  });
});
