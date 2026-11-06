// The pure rules of the agents feature: autosave, the setup checklist, the create form and the
// fixed choices.
import { describe, expect, it } from 'vitest';

import type { AgentSettings } from '../api/agents-api';
import { changedSettings, saveStatus, validateSettings } from './autosave';
import { BUILT_IN_PURPOSES, MODELS, purposeLabel, toModel, withCurrent } from './choices';
import { checkNewAgent } from './new-agent';
import { toChecklist } from './setup-checklist';

const saved: AgentSettings = {
  name: 'Front desk',
  purpose: 'support',
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
  it('accepts the saved settings, and leaves free text the schema allows to the server', () => {
    expect(validateSettings(saved)).toEqual({});
    expect(validateSettings({ ...saved, jurisdiction: ' us_ca ' })).toEqual({});
    expect(validateSettings({ ...saved, jurisdiction: 'Mars' })).toEqual({});
  });

  it("names each field that breaks the contract's request schema", () => {
    expect(
      validateSettings({
        ...saved,
        name: '',
        turnDetection: { silenceMs: 100, bargeIn: true },
      }),
    ).toEqual({
      name: 'Enter a name of up to 80 characters.',
      turnDetection: 'Use a pause between 200 and 2000 ms.',
    });
    expect(validateSettings({ ...saved, name: 'a'.repeat(81) })).toHaveProperty('name');
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

const CLINIC = {
  value: '019a5a5a-0000-7000-8000-000000000001',
  label: 'Clinic bookings',
  useCase: 'booking' as const,
  system: false,
};
const CHOICES = [...BUILT_IN_PURPOSES, CLINIC];

describe('checkNewAgent', () => {
  it('builds the agent from the three fields, a built-in purpose as its use case', () => {
    expect(
      checkNewAgent({ name: 'Front desk', purpose: 'booking', voice: 'af_heart' }, CHOICES),
    ).toEqual({
      agent: { name: 'Front desk', useCase: 'booking', voice: 'af_heart' },
    });
  });

  it('sends another purpose as its ID, with its compliance class as the use case', () => {
    expect(
      checkNewAgent({ name: 'Clinic desk', purpose: CLINIC.value, voice: 'af_heart' }, CHOICES),
    ).toEqual({
      agent: {
        name: 'Clinic desk',
        useCase: 'booking',
        voice: 'af_heart',
        purposeId: CLINIC.value,
      },
    });
  });

  it('names every missing field', () => {
    expect(checkNewAgent({ name: ' ', purpose: '', voice: '' }, CHOICES)).toEqual({
      errors: {
        name: 'Enter a name.',
        purpose: 'Choose a purpose.',
        voice: 'Choose a voice.',
      },
    });
    expect(checkNewAgent({ name: 'Desk', purpose: 'booking', voice: '' }, CHOICES).errors).toEqual({
      voice: 'Choose a voice.',
    });
  });
});

describe('choices', () => {
  it('offers the six built-in purposes before the list loads, and two models', () => {
    expect(BUILT_IN_PURPOSES.map((purpose) => purpose.value)).toEqual([
      'support',
      'service',
      'booking',
      'survey',
      'telemarketing',
      'custom',
    ]);
    expect(MODELS.map((model) => model.value)).toEqual(['balanced', 'fast']);
  });

  it('narrows select values back to their types and names purposes', () => {
    expect(toModel('fast')).toBe('fast');
    expect(toModel('huge')).toBeUndefined();
    expect(purposeLabel(CHOICES, 'telemarketing')).toBe('Outbound sales');
    expect(purposeLabel(CHOICES, CLINIC.value)).toBe('Clinic bookings');
    expect(purposeLabel(CHOICES, 'unknown')).toBe('unknown');
  });

  it('keeps the stored value in a picker even before the catalog loads', () => {
    const catalog = [{ value: 'af_heart', label: 'Heart' }];

    expect(withCurrent(undefined, 'af_heart')).toEqual([{ value: 'af_heart', label: 'af_heart' }]);
    expect(withCurrent(catalog, 'af_heart')).toEqual(catalog);
    expect(withCurrent(catalog, 'old')).toEqual([{ value: 'old', label: 'old' }, ...catalog]);
  });
});
