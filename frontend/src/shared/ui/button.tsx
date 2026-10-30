import { Button as ButtonPrimitive } from '@base-ui/react/button';
import { cva, type VariantProps } from 'class-variance-authority';
import type { ComponentProps } from 'react';

import { cn } from '../lib/cn';

/**
 * The only button in the app (shadcn pattern on Base UI). Variants are fixed (similarity):
 * - primary: the one main action on a screen (Von Restorff); at most one visible.
 * - secondary: the default for every other action.
 * - ghost: quiet actions inside lists and toolbars.
 * - danger: destructive actions; pair with a confirmation dialog.
 * Every variant keeps the 44 × 44 px minimum target (Fitts).
 */
const buttonVariants = cva(
  [
    'inline-flex min-h-target min-w-target cursor-pointer items-center justify-center gap-2',
    'rounded-md px-4 text-sm font-medium transition-colors select-none',
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus',
    'data-disabled:cursor-not-allowed data-disabled:opacity-50',
  ],
  {
    variants: {
      variant: {
        primary: 'bg-primary text-on-primary shadow-sm hover:bg-primary-hover',
        secondary: 'border border-line bg-canvas text-ink hover:bg-hover',
        ghost: 'text-ink hover:bg-hover',
        danger: 'bg-danger text-on-danger hover:bg-danger-hover',
      },
    },
    defaultVariants: { variant: 'secondary' },
  },
);

type ButtonVariant = NonNullable<VariantProps<typeof buttonVariants>['variant']>;

/**
 * The look of a Button for a link that navigates (Base UI: links keep link semantics, so they
 * are styled, not rendered through Button). Spread it on the router's link:
 * `<Link to="/agents/new" {...buttonLook('primary')}>Create agent</Link>`.
 */
export function buttonLook(variant: ButtonVariant = 'secondary') {
  return { 'data-variant': variant, className: buttonVariants({ variant }) };
}

export interface ButtonProps extends Omit<ComponentProps<typeof ButtonPrimitive>, 'className'> {
  variant?: ButtonVariant;
  className?: string;
}

export function Button({
  variant = 'secondary',
  className,
  type = 'button',
  ...props
}: ButtonProps) {
  return (
    <ButtonPrimitive
      data-slot="button"
      data-variant={variant}
      type={type}
      className={cn(buttonVariants({ variant }), className)}
      {...props}
    />
  );
}
