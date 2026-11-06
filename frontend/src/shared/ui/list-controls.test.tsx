// Controls for lists that grow: a choice that turns searchable past seven options, a list of
// phrases edited in place, section links, and the modal layer that keeps one primary action.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { Button } from './button';
import { ChoiceField, MAX_SELECT_OPTIONS } from './choice-field';
import { FormDialog } from './form-dialog';
import { RadioGroupField } from './radio-group-field';
import { SectionNav, sectionLinkClassName } from './section-nav';
import { TextListField } from './text-list-field';

function options(count: number) {
  return Array.from({ length: count }, (_, at) => ({
    value: `p${String(at + 1)}`,
    label: `Purpose ${String(at + 1)}`,
  }));
}

describe('ChoiceField', () => {
  it('is a native select up to seven options', () => {
    render(
      <ChoiceField
        label="Purpose"
        value="p1"
        onValueChange={vi.fn<(value: string) => void>()}
        options={options(MAX_SELECT_OPTIONS)}
      />,
    );

    const select = screen.getByRole('combobox', { name: 'Purpose' });
    expect(select.tagName).toBe('SELECT');
    expect(within(select).getAllByRole('option')).toHaveLength(7);
  });

  it('turns searchable past seven: type to filter, choose with the keyboard', async () => {
    const onValueChange = vi.fn<(value: string) => void>();
    render(
      <ChoiceField
        label="Purpose"
        value=""
        onValueChange={onValueChange}
        options={options(MAX_SELECT_OPTIONS + 3)}
        placeholder="Search purposes"
        description="Built-in ones first."
      />,
    );

    const input = screen.getByRole('combobox', { name: 'Purpose' });
    expect(input.tagName).toBe('INPUT');
    expect(input).toHaveAccessibleDescription('Built-in ones first.');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    expect(await screen.findAllByRole('option')).toHaveLength(10);
    fireEvent.change(input, { target: { value: 'Purpose 10' } });
    const list = await screen.findByRole('listbox');
    expect(
      within(list)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toEqual(['Purpose 10']);
    fireEvent.click(within(list).getByRole('option', { name: 'Purpose 10' }));
    await waitFor(() => {
      expect(onValueChange).toHaveBeenCalledWith('p10');
    });
  });

  it('says when nothing matches', async () => {
    render(
      <ChoiceField
        label="Purpose"
        value="p2"
        onValueChange={vi.fn<(value: string) => void>()}
        options={options(9)}
      />,
    );

    const input = screen.getByRole('combobox', { name: 'Purpose' });
    expect(input).toHaveValue('Purpose 2');
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.change(input, { target: { value: 'zebra' } });
    expect(await screen.findByText('Nothing matches. Try another word.')).toBeVisible();
  });
});

function renderList(items: string[], readOnly = false) {
  const onItemsChange = vi.fn<(items: string[]) => void>();
  render(
    <TextListField
      label="Allowed topics"
      noun="topic"
      items={items}
      onItemsChange={onItemsChange}
      description="None means any topic."
      maxItems={3}
      maxLength={200}
      empty="Any topic."
      readOnly={readOnly}
    />,
  );
  return onItemsChange;
}

describe('TextListField', () => {
  it('says what an empty list means and adds a trimmed item with Enter or Add', () => {
    const onItemsChange = renderList([]);

    expect(screen.getByRole('list', { name: 'Allowed topics' })).toHaveTextContent('Any topic.');
    const input = screen.getByRole('textbox', { name: 'New topic' });
    expect(input).toHaveAccessibleDescription('None means any topic.');
    expect(screen.getByRole('button', { name: 'Add topic' })).toBeDisabled();
    fireEvent.change(input, { target: { value: '  Appointments ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onItemsChange).toHaveBeenLastCalledWith(['Appointments']);
    expect(input).toHaveValue('');
    fireEvent.change(input, { target: { value: 'Prices' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add topic' }));
    expect(onItemsChange).toHaveBeenLastCalledWith(['Prices']);
  });

  it('skips a repeat, ignoring case, and removes an item', () => {
    const onItemsChange = renderList(['Appointments', 'Prices']);
    const input = screen.getByRole('textbox', { name: 'New topic' });

    fireEvent.change(input, { target: { value: 'appointments' } });
    fireEvent.keyDown(input, { key: 'a' });
    fireEvent.keyDown(input, { key: 'Enter' });
    fireEvent.click(screen.getByRole('button', { name: 'Add topic' }));
    expect(onItemsChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove Appointments' }));
    expect(onItemsChange).toHaveBeenCalledWith(['Prices']);
  });

  it('hides the input when full or read-only', () => {
    renderList(['A', 'B', 'C']);
    expect(screen.queryByRole('textbox', { name: 'New topic' })).not.toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /^Remove / })).toHaveLength(3);
  });

  it('shows the items without actions when read-only', () => {
    renderList(['A'], true);
    expect(screen.getByRole('list', { name: 'Allowed topics' })).toHaveTextContent('A');
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });
});

describe('SectionNav', () => {
  it('is a named navigation of section links', () => {
    render(
      <SectionNav label="Purpose sections">
        <li>
          <a href="?section=tools" aria-current="page" className={sectionLinkClassName}>
            Tools
          </a>
        </li>
      </SectionNav>,
    );

    const nav = screen.getByRole('navigation', { name: 'Purpose sections' });
    expect(within(nav).getByRole('link', { name: 'Tools' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });
});

describe('RadioGroupField', () => {
  it('is a named group of radios with one chosen', () => {
    const onValueChange = vi.fn<(value: 'keep' | 'replace') => void>();
    render(
      <RadioGroupField
        label="Your plan"
        value="keep"
        onValueChange={onValueChange}
        options={[
          { value: 'keep', label: 'Keep my plan' },
          { value: 'replace', label: 'Replace my plan' },
        ]}
      />,
    );

    const group = screen.getByRole('group', { name: 'Your plan' });
    expect(within(group).getByRole('radio', { name: 'Keep my plan' })).toBeChecked();
    fireEvent.click(within(group).getByRole('radio', { name: 'Replace my plan' }));
    expect(onValueChange).toHaveBeenCalledWith('replace');
  });
});

describe('the modal layer', () => {
  it('shows a primary behind an open dialog as secondary, and the dialog submit as primary', async () => {
    render(
      <>
        <Button variant="primary">Save version 2</Button>
        <FormDialog
          triggerLabel="Add escalation rule"
          title="Add escalation rule"
          submitLabel="Add rule"
          onSubmit={() => true}
        >
          <input aria-label="When" />
        </FormDialog>
      </>,
    );
    const save = screen.getByRole('button', { name: 'Save version 2' });
    expect(save).toHaveAttribute('data-variant', 'primary');

    fireEvent.click(screen.getByRole('button', { name: 'Add escalation rule' }));
    const dialog = await screen.findByRole('dialog', { name: 'Add escalation rule' });

    expect(screen.getByRole('button', { name: 'Save version 2', hidden: true })).toHaveAttribute(
      'data-variant',
      'secondary',
    );
    expect(within(dialog).getByRole('button', { name: 'Add rule' })).toHaveAttribute(
      'data-variant',
      'primary',
    );
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add rule' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
    expect(save).toHaveAttribute('data-variant', 'primary');
  });
});
