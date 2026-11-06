import { X } from 'lucide-react';
import { useId, useState } from 'react';

import { Button } from './button';
import { controlClassName } from './field';

interface TextListFieldProps {
  /** The list's name, e.g. "Allowed topics": its legend and the list's accessible name. */
  label: string;
  /** One item, in a sentence: "topic" names "New topic", "Add topic", "Remove …". */
  noun: string;
  items: readonly string[];
  onItemsChange: (items: string[]) => void;
  description?: string | undefined;
  /** Most items; the input hides once it is reached. */
  maxItems: number;
  /** Longest item, in characters. */
  maxLength: number;
  /** What the empty list says. */
  empty?: string | undefined;
  /** Shown, not changed. */
  readOnly?: boolean | undefined;
}

/**
 * A short list of phrases to edit in place: each item with its own Remove, then one input and
 * Add (Enter adds too). Repeats, ignoring case, are not added (Postel: the list stays clean).
 */
export function TextListField({
  label,
  noun,
  items,
  onItemsChange,
  description,
  maxItems,
  maxLength,
  empty,
  readOnly = false,
}: TextListFieldProps) {
  const [text, setText] = useState('');
  const inputId = useId();
  const descriptionId = `${inputId}-description`;
  const add = () => {
    const item = text.trim();
    if (item === '') return;
    const known = items.some((existing) => existing.toLowerCase() === item.toLowerCase());
    if (!known) onItemsChange([...items, item]);
    setText('');
  };
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="text-sm font-medium">{label}</legend>
      {description !== undefined && (
        <p id={descriptionId} className="text-sm text-muted">
          {description}
        </p>
      )}
      <ul aria-label={label} className="flex flex-col gap-1">
        {items.length === 0 && empty !== undefined && (
          <li className="text-sm text-muted">{empty}</li>
        )}
        {items.map((item) => (
          <li
            key={item}
            className="flex min-h-target items-center justify-between gap-2 rounded-md bg-surface pl-3"
          >
            <span className="min-w-0 py-2 text-sm break-words">{item}</span>
            {!readOnly && (
              <Button
                variant="ghost"
                aria-label={`Remove ${item}`}
                onClick={() => {
                  onItemsChange(items.filter((existing) => existing !== item));
                }}
              >
                <X aria-hidden className="size-4" />
              </Button>
            )}
          </li>
        ))}
      </ul>
      {!readOnly && items.length < maxItems && (
        <div className="flex items-end gap-2">
          <div className="flex min-w-0 flex-1 flex-col gap-1">
            <label htmlFor={inputId} className="sr-only">
              {`New ${noun}`}
            </label>
            <input
              id={inputId}
              value={text}
              maxLength={maxLength}
              placeholder={`New ${noun}`}
              aria-describedby={description === undefined ? undefined : descriptionId}
              onChange={(event) => {
                setText(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key !== 'Enter') return;
                event.preventDefault();
                add();
              }}
              className={controlClassName}
            />
          </div>
          <Button onClick={add} disabled={text.trim() === ''}>
            {`Add ${noun}`}
          </Button>
        </div>
      )}
    </fieldset>
  );
}
