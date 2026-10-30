import { RuleTester } from 'oxlint/plugins-dev';
import { describe, expect, it } from 'vitest';

import { utilityOf } from './class-names.ts';
import plugin from './vak-ux.ts';

RuleTester.describe = describe;
RuleTester.it = it;

const tester = new RuleTester({ languageOptions: { parserOptions: { lang: 'tsx' } } });

tester.run('no-raw-button', plugin.rules['no-raw-button'], {
  valid: [
    'const a = <Button variant="primary">Save</Button>;',
    'const a = <input type="text" />;',
    'const a = <input type={kind} />;',
    'const a = <ui.button />;',
  ],
  invalid: [
    {
      code: 'const a = <button type="button">Save</button>;',
      errors: [{ messageId: 'rawButton' }],
    },
    { code: 'const a = <input type="submit" />;', errors: [{ messageId: 'rawButton' }] },
    { code: 'const a = <input type="reset" value="x" />;', errors: [{ messageId: 'rawButton' }] },
  ],
});

tester.run('no-arbitrary-values', plugin.rules['no-arbitrary-values'], {
  valid: [
    'const a = <div className="flex gap-4 p-6" />;',
    'const a = <a className="aria-[current=page]:bg-selected [&_svg]:size-4" />;',
    "const a = cn('p-4', isOpen && 'bg-hover');",
    "const a = <Link activeProps={{ className: 'font-semibold' }} />;",
    "const label = 'p-[13px] is only text here';",
  ],
  invalid: [
    { code: 'const a = <div className="p-[13px]" />;', errors: [{ messageId: 'arbitrary' }] },
    {
      code: 'const a = <div className="hover:bg-(--brand)" />;',
      errors: [{ messageId: 'arbitrary' }],
    },
    {
      code: 'const a = <div className={`flex ${x} w-[3rem]`} />;',
      errors: [{ messageId: 'arbitrary' }],
    },
    {
      code: "const a = <div className={open ? 'm-[2px]' : 'm-2'} />;",
      errors: [{ messageId: 'arbitrary' }],
    },
    {
      code: "const a = <div className={cn('p-4', 'gap-[7px]')} />;",
      errors: [{ messageId: 'arbitrary' }],
    },
    {
      code: "const v = cva('p-4', { variants: { size: { lg: 'h-[51px]' } } });",
      errors: [{ messageId: 'arbitrary' }],
    },
    {
      code: "const a = <Link activeProps={{ className: 'top-[1px]' }} />;",
      errors: [{ messageId: 'arbitrary' }],
    },
  ],
});

tester.run('primary-colour-only-in-button', plugin.rules['primary-colour-only-in-button'], {
  valid: [
    'const a = <div className="bg-canvas text-ink" />;',
    'const a = <Button variant="primary" />;',
    "const v = cva('p-4', { defaultVariants: { variant: 'primary' } });",
  ],
  invalid: [
    { code: 'const a = <div className="bg-primary" />;', errors: [{ messageId: 'primary' }] },
    {
      code: 'const a = <div className="hover:text-on-primary" />;',
      errors: [{ messageId: 'primary' }],
    },
    { code: "const a = cn('border-primary-hover/50');", errors: [{ messageId: 'primary' }] },
  ],
});

describe('utilityOf', () => {
  it('strips variants but keeps colons inside brackets', () => {
    expect(utilityOf('hover:md:bg-primary/50')).toBe('bg-primary/50');
    expect(utilityOf('[&:hover]:p-2')).toBe('p-2');
    expect(utilityOf('!p-2')).toBe('p-2');
  });
});
