import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { buttonLook } from './button';
import { CheckboxField } from './checkbox-field';
import { ConfirmDialog } from './confirm-dialog';
import { ProgressBar } from './progress-bar';
import { SelectField } from './select-field';
import { TextField } from './text-field';

describe('TextField', () => {
  it('ties the label, help and error to the input', () => {
    const onValueChange = vi.fn<(value: string) => void>();
    render(
      <TextField
        label="Name"
        value="Desk"
        onValueChange={onValueChange}
        description="Shown in the list."
        error="Enter a name."
      />,
    );

    const input = screen.getByRole('textbox', { name: 'Name' });
    expect(input).toHaveAccessibleDescription('Shown in the list. Enter a name.');
    expect(input).toHaveAttribute('aria-invalid', 'true');
    fireEvent.change(input, { target: { value: 'Front desk' } });
    expect(onValueChange).toHaveBeenCalledWith('Front desk');
  });

  it('is valid and undescribed without help or error', () => {
    render(<TextField label="Name" value="" onValueChange={vi.fn<(value: string) => void>()} />);

    const input = screen.getByRole('textbox', { name: 'Name' });
    expect(input).not.toHaveAttribute('aria-invalid');
    expect(input).not.toHaveAttribute('aria-describedby');
  });
});

describe('SelectField', () => {
  const options = [
    { value: 'support', label: 'Customer support' },
    { value: 'survey', label: 'Survey' },
  ];

  it('shows a placeholder that cannot be chosen, then the options', () => {
    const onValueChange = vi.fn<(value: string) => void>();
    render(
      <SelectField
        label="Purpose"
        value=""
        onValueChange={onValueChange}
        options={options}
        placeholder="Choose a purpose"
      />,
    );

    const select = screen.getByRole('combobox', { name: 'Purpose' });
    const choices = within(select).getAllByRole('option');
    expect(choices.map((option) => option.textContent)).toEqual([
      'Choose a purpose',
      'Customer support',
      'Survey',
    ]);
    expect(choices[0]).toBeDisabled();
    fireEvent.change(select, { target: { value: 'survey' } });
    expect(onValueChange).toHaveBeenCalledWith('survey');
  });

  it('can be disabled', () => {
    render(
      <SelectField
        label="Voice"
        value=""
        onValueChange={vi.fn<(value: string) => void>()}
        options={options}
        disabled
      />,
    );

    expect(screen.getByRole('combobox', { name: 'Voice' })).toBeDisabled();
  });
});

describe('CheckboxField', () => {
  it('toggles through its label', () => {
    const onCheckedChange = vi.fn<(checked: boolean) => void>();
    render(
      <CheckboxField
        label="Callers can interrupt"
        checked={false}
        onCheckedChange={onCheckedChange}
        description="Barge-in"
      />,
    );

    const checkbox = screen.getByRole('checkbox', { name: 'Callers can interrupt' });
    expect(checkbox).toHaveAccessibleDescription('Barge-in');
    fireEvent.click(screen.getByText('Callers can interrupt'));
    expect(onCheckedChange).toHaveBeenCalledWith(true);
  });
});

describe('ProgressBar', () => {
  it('is a named progressbar with its value, clamped to 0..100', () => {
    render(<ProgressBar label="Setup progress" value={25} />);

    const bar = screen.getByRole('progressbar', { name: 'Setup progress' });
    expect(bar).toHaveAttribute('aria-valuenow', '25');
    expect(bar).toHaveAttribute('aria-valuetext', '25 %');
    expect(screen.getByText('25 %')).toBeVisible();
  });

  it('never reports more than 100 %', () => {
    render(<ProgressBar label="Setup progress" value={140} />);

    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
  });
});

describe('ConfirmDialog', () => {
  it('asks in an alertdialog and acts only on confirm', async () => {
    const onConfirm = vi.fn<() => void>();
    render(
      <ConfirmDialog
        triggerLabel="Archive agent"
        title="Archive Front desk?"
        description="It leaves the list."
        confirmLabel="Archive"
        onConfirm={onConfirm}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Archive agent' }));
    const dialog = await screen.findByRole('alertdialog', { name: 'Archive Front desk?' });
    expect(dialog).toHaveAccessibleDescription('It leaves the list.');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Archive' }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it('closes on Cancel without acting', async () => {
    const onConfirm = vi.fn<() => void>();
    render(
      <ConfirmDialog
        triggerLabel="Archive agent"
        title="Archive Front desk?"
        description="It leaves the list."
        confirmLabel="Archive"
        onConfirm={onConfirm}
        pending
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Archive agent' }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByRole('button', { name: 'Archive' })).toHaveAttribute(
      'aria-busy',
      'true',
    );
    fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

describe('buttonLook', () => {
  it('gives a link the button variant and its 44 px target', () => {
    const look = buttonLook('primary');

    expect(look['data-variant']).toBe('primary');
    expect(look.className).toContain('min-h-target');
    expect(buttonLook()['data-variant']).toBe('secondary');
  });
});
