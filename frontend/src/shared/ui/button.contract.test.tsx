// Contract: shared/ui Button, part of app shell v1. Pinned by the signed tag contract/app-shell/v1.
// Never edit this file; to change the contract, add a v2 and get it signed.
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Button } from './button';

import tokensCss from './tokens.css?raw';

/**
 * Resolves the size an element's classes give it on one axis, in CSS px, through the spacing
 * tokens in tokens.css (for example `min-h-target` -> `--spacing-target: 2.75rem` -> 44).
 * Returns the largest match, or 0 when no class sets that axis from a token.
 */
function tokenSizePx(element: HTMLElement, axis: 'height' | 'width'): number {
  const prefixes = axis === 'height' ? ['min-h-', 'h-', 'size-'] : ['min-w-', 'w-', 'size-'];
  const sizes = [...element.classList].flatMap((className) => {
    const prefix = prefixes.find((candidate) => className.startsWith(candidate));
    if (prefix === undefined) return [];
    const token = className.slice(prefix.length).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const match = new RegExp(`--spacing-${token}:\\s*([\\d.]+)(rem|px);`).exec(tokensCss);
    return match === null ? [] : [Number(match[1]) * (match[2] === 'rem' ? 16 : 1)];
  });
  return Math.max(0, ...sizes);
}

describe('Button contract v1', () => {
  it('renders a native button with its accessible name', () => {
    render(<Button>Cancel</Button>);
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInstanceOf(HTMLButtonElement);
  });

  it('defaults to type="button", so it never submits a form by accident', () => {
    render(<Button>Cancel</Button>);
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveAttribute('type', 'button');
  });

  it('exposes its variant as data-variant, and the primary variant exists', () => {
    render(<Button variant="primary">Create agent</Button>);
    expect(screen.getByRole('button', { name: 'Create agent' })).toHaveAttribute(
      'data-variant',
      'primary',
    );
  });

  it('is not primary by default, so a screen has a primary action only on purpose', () => {
    render(<Button>Cancel</Button>);
    const button = screen.getByRole('button', { name: 'Cancel' });
    expect(button).toHaveAttribute('data-variant');
    expect(button.getAttribute('data-variant')).not.toBe('primary');
  });

  it('meets the 44 × 44 CSS px minimum target through token-backed classes', () => {
    render(<Button variant="primary">Go</Button>);
    const button = screen.getByRole('button', { name: 'Go' });
    expect(tokenSizePx(button, 'height')).toBeGreaterThanOrEqual(44);
    expect(tokenSizePx(button, 'width')).toBeGreaterThanOrEqual(44);
  });
});
