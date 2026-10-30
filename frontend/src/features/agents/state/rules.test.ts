// The pure rules of the agents feature: autosave, the setup checklist, the create form and the
// fixed choices.
import { describe, expect, it } from 'vitest';

import type { AgentSettings } from '../api/agents-api';
import { changedSettings, saveStatus, validateSettings } from './autosave';
import { MODELS, PURPOSES, purposeLabel, toModel, toUseCase, withCurrent } from './choices';
import { checkNewAgent } from './new-agent';
import { toChecklist } from './setup-checklist';

const saved: AgentSettings = {
  name: 'Front desk',
  useCase: 'support',
  voice: 'af_heart',
  language: 'en-US',
  jurisdiction: 'US',
  model: 'balanced',
  turnDetection: { silenceMs: 500, bargeIn: true },
};

describe('changedSettings', () => {
  it('is empty when nothing changed, including an equal copy of turn detection', () => {
    expect(
      changedSettings(saved, { ...saved, turnDetection: { silenceMs: 500, bargeIn: true } }),
    ).toEqual({});
  });

  it('lists only the changed fields, with turn detection as a whole', () => {
    const draft = {
      ...saved,
      name: 'After hours',
      turnDetection: { silenceMs: 800, bargeIn: true },
    };

    expect(changedSettings(saved, draft)).toEqual({
      name: 'After hours',
      turnDetection: { silenceMs: 800, bargeIn: true },
    });
    expect(
      changedSettings(saved, { ...saved, turnDetection: { silenceMs: 500, bargeIn: false } }),
    ).toEqual({
      turnDetection: { silenceMs: 500, bargeIn: false },
    });
  });
});

describe('validateSettings', () => {
  it('accepts the saved settings and liberal jurisdiction input', () => {
    expect(validateSettings(saved)).toEqual({});
    expect(validateSettings({ ...saved, jurisdiction: ' us_ca ' })).toEqual({});
  });

  it('names each field to fix', () => {
    expect(
      validateSettings({
        ...saved,
        name: '   ',
        jurisdiction: 'Mars',
        turnDetection: { silenceMs: 100, bargeIn: true },
      }),
    ).toEqual({
      name: 'Enter a name.',
      jurisdiction: 'Use a region code such as US, US-CA or UK.',
      turnDetection: 'Use a pause between 200 and 2000 ms.',
    });
    expect(validateSettings({ ...saved, name: 'Front\ndesk' }).name).toBe(
      'Remove line breaks and control characters.',
    );
    expect(
      validateSettings({ ...saved, turnDetection: { silenceMs: Number.NaN, bargeIn: true } }),
    ).toHaveProperty('turnDetection');
  });
});

describe('saveStatus', () => {
  const quiet = { dirty: false, saving: false, invalid: false, failed: false };

  it('says saved only when nothing is pending', () => {
    expect(saveStatus(quiet)).toBe('saved');
    expect(saveStatus({ ...quiet, dirty: true })).toBe('unsaved');
    expect(saveStatus({ ...quiet, dirty: true, saving: true })).toBe('saving');
    expect(saveStatus({ ...quiet, dirty: true, failed: true })).toBe('failed');
  });

  it('puts an invalid field first', () => {
    expect(saveStatus({ dirty: true, saving: true, invalid: true, failed: true })).toBe('invalid');
  });
});

describe('toChecklist', () => {
  it('labels the steps and marks the first unfinished one as next', () => {
    const checklist = toChecklist({
      percent: 25,
      steps: [
        { step: 'provision', done: true },
        { step: 'plan', done: false },
        { step: 'test', done: false },
        { step: 'publish', done: false },
      ],
      nextStep: 'plan',
    });

    expect(checklist.percent).toBe(25);
    expect(checklist.steps.map(({ label, state }) => [label, state])).toEqual([
      ['Provision', 'done'],
      ['Plan', 'next'],
      ['Test', 'todo'],
      ['Publish', 'todo'],
    ]);
  });

  it('has no next step when everything is done', () => {
    const checklist = toChecklist({
      percent: 100,
      steps: [
        { step: 'provision', done: true },
        { step: 'publish', done: true },
      ],
      nextStep: null,
    });

    expect(checklist.steps.every((step) => step.state === 'done')).toBe(true);
  });
});

describe('checkNewAgent', () => {
  it('builds the agent from the three fields', () => {
    expect(checkNewAgent({ name: 'Front desk', purpose: 'booking', voice: 'af_heart' })).toEqual({
      agent: { name: 'Front desk', useCase: 'booking', voice: 'af_heart' },
    });
  });

  it('names every missing field', () => {
    expect(checkNewAgent({ name: ' ', purpose: '', voice: '' })).toEqual({
      errors: {
        name: 'Enter a name.',
        useCase: 'Choose a purpose.',
        voice: 'Choose a voice.',
      },
    });
    expect(checkNewAgent({ name: 'Desk', purpose: 'booking', voice: '' }).errors).toEqual({
      voice: 'Choose a voice.',
    });
  });
});

describe('choices', () => {
  it('offers at most seven purposes (Hick) and two models', () => {
    expect(PURPOSES.length).toBeLessThanOrEqual(7);
    expect(MODELS.map((model) => model.value)).toEqual(['balanced', 'fast']);
  });

  it('narrows select values back to their types', () => {
    expect(toUseCase('survey')).toBe('survey');
    expect(toUseCase('sales')).toBeUndefined();
    expect(toModel('fast')).toBe('fast');
    expect(toModel('huge')).toBeUndefined();
    expect(purposeLabel('telemarketing')).toBe('Outbound sales');
  });

  it('keeps the stored value in a picker even before the catalog loads', () => {
    const catalog = [{ value: 'af_heart', label: 'Heart' }];

    expect(withCurrent(undefined, 'af_heart')).toEqual([{ value: 'af_heart', label: 'af_heart' }]);
    expect(withCurrent(catalog, 'af_heart')).toEqual(catalog);
    expect(withCurrent(catalog, 'old')).toEqual([{ value: 'old', label: 'old' }, ...catalog]);
  });
});
