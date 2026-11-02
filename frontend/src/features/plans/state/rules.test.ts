import { describe, expect, it } from 'vitest';

import type { PlanDraft } from '../api/plans-api';
import { columnIndex, columnsOf } from './layout';
import {
  addPath,
  addStep,
  deletePath,
  deleteStep,
  joinOpening,
  nextId,
  sameDraft,
  splitOpening,
  updateStep,
} from './plan-edits';

const LOCKED = [
  { key: 'ai_disclosure', text: "You're talking to an AI." },
  { key: 'recording_notice', text: 'This call is recorded.' },
];

function draft(): PlanDraft {
  return {
    steps: [
      { id: 'start', kind: 'start', label: 'Greeting', text: 'Hi.' },
      { id: 'ask', kind: 'ask', label: 'Ask', text: '' },
      { id: 'bye', kind: 'end', label: 'Bye', text: '' },
    ],
    paths: [
      { id: 'p1', from: 'start', to: 'ask', condition: 'Always' },
      { id: 'p2', from: 'ask', to: 'bye', condition: 'Done' },
    ],
  };
}

describe('plan edits', () => {
  it('picks the first free ID for a prefix', () => {
    expect(nextId('step', [])).toBe('step-1');
    expect(nextId('step', ['step-1', 'step-3'])).toBe('step-2');
  });

  it('adds a trimmed step at the end, with a fresh ID and no text', () => {
    const { draft: added, stepId } = addStep(draft(), { label: '  Wrap up ', kind: 'say' });

    expect(stepId).toBe('step-1');
    expect(added.steps.at(-1)).toEqual({ id: 'step-1', kind: 'say', label: 'Wrap up', text: '' });
    expect(added.paths).toEqual(draft().paths);
  });

  it('changes one step and leaves the others', () => {
    const changed = updateStep(draft(), 'ask', { label: 'Ask the question', text: 'What?' });

    expect(changed.steps[1]).toEqual({
      id: 'ask',
      kind: 'ask',
      label: 'Ask the question',
      text: 'What?',
    });
    expect(changed.steps[0]).toEqual(draft().steps[0]);
  });

  it('deletes a step with every path to and from it', () => {
    const deleted = deleteStep(draft(), 'ask');

    expect(deleted.steps.map((step) => step.id)).toEqual(['start', 'bye']);
    expect(deleted.paths).toEqual([]);
  });

  it('adds and removes paths', () => {
    const added = addPath(draft(), { from: 'bye', to: 'start', condition: ' Again ' });

    expect(added.paths.at(-1)).toEqual({
      id: 'path-1',
      from: 'bye',
      to: 'start',
      condition: 'Again',
    });
    expect(deletePath(added, 'path-1')).toEqual(draft());
  });

  it('splits an opening into the locked texts and the greeting, and joins them back', () => {
    const opening = joinOpening(LOCKED, ' How can I help? ');

    expect(opening).toBe("You're talking to an AI. This call is recorded. How can I help?");
    expect(splitOpening(opening, LOCKED)).toEqual({ intact: true, greeting: 'How can I help?' });
    expect(splitOpening('Hello there.', LOCKED)).toEqual({
      intact: false,
      greeting: 'Hello there.',
    });
    expect(joinOpening(LOCKED, '')).toBe("You're talking to an AI. This call is recorded.");
    expect(splitOpening('Hi.', [])).toEqual({ intact: true, greeting: 'Hi.' });
  });

  it('compares drafts by content and order', () => {
    expect(sameDraft(draft(), draft())).toBe(true);
    expect(sameDraft(draft(), deletePath(draft(), 'p1'))).toBe(false);
  });
});

describe('layout', () => {
  it('puts each step in the column of its distance from the start', () => {
    const columns = columnsOf(draft());

    expect(columns.map((column) => column.map((step) => step.id))).toEqual([
      ['start'],
      ['ask'],
      ['bye'],
    ]);
    expect(columnIndex(columns)).toEqual(
      new Map([
        ['start', 0],
        ['ask', 1],
        ['bye', 2],
      ]),
    );
  });

  it('keeps a step where the walk first reaches it, and ignores paths to missing steps', () => {
    const looped = addPath(addPath(draft(), { from: 'bye', to: 'start', condition: 'Again' }), {
      from: 'start',
      to: 'ghost',
      condition: 'Lost',
    });

    expect(columnsOf(looped).map((column) => column.map((step) => step.id))).toEqual([
      ['start'],
      ['ask'],
      ['bye'],
    ]);
  });

  it('gathers the steps nothing reaches in a last column', () => {
    const { draft: lonely } = addStep(draft(), { label: 'Lonely', kind: 'say' });
    const noStart = { ...draft(), steps: draft().steps.filter((step) => step.kind !== 'start') };

    expect(
      columnsOf(lonely)
        .at(-1)
        ?.map((step) => step.id),
    ).toEqual(['step-1']);
    expect(columnsOf(noStart).map((column) => column.map((step) => step.id))).toEqual([
      ['ask', 'bye'],
    ]);
    expect(columnsOf({ steps: [], paths: [] })).toEqual([]);
  });
});
