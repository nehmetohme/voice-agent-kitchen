// The purpose editor's pure rules: what a save sends, tool and guardrail edits, the version
// standing, and the fixed choices.
import { describe, expect, it } from 'vitest';

import type { HttpToolDraft, PurposeSettings, ToolSettings } from '../api/purpose-models';
import { actionLabel, choose, classLabel, METHODS } from './choices';
import {
  changedSections,
  checkHttpTool,
  hostAllowed,
  hostOf,
  newEscalation,
  newHttpTool,
  newToolInput,
  putHttpTool,
  removeHttpTool,
  toggleBuiltin,
} from './purpose-edits';
import { SECTIONS, sectionLabel } from './sections';
import { versionState } from './version-state';

const SETTINGS: PurposeSettings = {
  overview: { name: 'Clinic', description: '', complianceClass: 'booking', persona: 'Book.' },
  tools: { builtins: ['mark_do_not_call'], http: [] },
  guardrails: { allowedTopics: [], never: [], escalations: [], maxCallMinutes: 30, neverAsk: [] },
  compliance: {
    optOutPhrases: [],
    forbiddenClaims: [],
    recordingsDays: null,
    transcriptsDays: null,
  },
};

function tool(name: string, overrides: Partial<HttpToolDraft> = {}): HttpToolDraft {
  return {
    ...newHttpTool(3000),
    name,
    description: 'Books a slot.',
    url: 'https://calendar.example.com/v1/slots',
    ...overrides,
  };
}

describe('changedSections', () => {
  it('sends only the sections that differ, each whole', () => {
    const draft = {
      ...SETTINGS,
      guardrails: { ...SETTINGS.guardrails, allowedTopics: ['Appointments'] },
    };

    expect(changedSections(SETTINGS, SETTINGS)).toEqual({});
    expect(changedSections(SETTINGS, draft)).toEqual({ guardrails: draft.guardrails });
  });
});

describe('tool edits', () => {
  it('turns a built-in tool on and off, but never the locked one', () => {
    const on = toggleBuiltin(SETTINGS.tools, 'end_call', true, false);

    expect(on.builtins).toEqual(['mark_do_not_call', 'end_call']);
    expect(toggleBuiltin(on, 'end_call', false, false).builtins).toEqual(['mark_do_not_call']);
    expect(toggleBuiltin(on, 'mark_do_not_call', false, true)).toBe(on);
  });

  it('adds, replaces and removes HTTP tools by name', () => {
    const added = putHttpTool(SETTINGS.tools, tool('book_slot'), null);
    const twice = putHttpTool(added, tool('find_slot'), null);
    const replaced = putHttpTool(twice, tool('book_slot', { method: 'PUT' }), 'book_slot');

    expect(twice.http.map((http) => http.name)).toEqual(['book_slot', 'find_slot']);
    expect(replaced.http[0]?.method).toBe('PUT');
    expect(replaced.http).toHaveLength(2);
    expect(removeHttpTool(replaced, 'book_slot').http.map((http) => http.name)).toEqual([
      'find_slot',
    ]);
    const unknown: ToolSettings = putHttpTool(added, tool('other'), 'missing');
    expect(unknown.http).toHaveLength(2);
  });

  it('starts a tool as a POST that confirms first, and an input as required text', () => {
    expect(newHttpTool(2500)).toMatchObject({ method: 'POST', timeoutMs: 2500, confirm: true });
    expect(newToolInput()).toEqual({ name: '', type: 'string', description: '', required: true });
    expect(newEscalation()).toEqual({ when: '', action: 'transfer', message: '' });
  });
});

describe('checkHttpTool', () => {
  it('passes a complete tool', () => {
    expect(checkHttpTool(tool('book_slot'), [], ['end_call'])).toEqual({});
  });

  it('names what to fix, by field', () => {
    const bad = tool('Book slot', { description: ' ', url: 'http://calendar' });

    expect(checkHttpTool(bad, [], [])).toEqual({
      name: 'Use lowercase letters, digits and _, starting with a letter.',
      description: 'Say what the tool does.',
      url: 'Use an https:// address with a host name.',
    });
  });

  it('refuses a name another tool has, built-in or not', () => {
    expect(checkHttpTool(tool('end_call'), [], ['end_call']).name).toBe(
      'Another tool has this name.',
    );
    expect(checkHttpTool(tool('book_slot'), ['book_slot'], []).name).toBe(
      'Another tool has this name.',
    );
  });

  it('wants each input named, once', () => {
    const input = { ...newToolInput(), name: 'date' };

    expect(checkHttpTool(tool('t', { inputs: [{ ...input, name: 'Date' }] }), [], []).inputs).toBe(
      'Name each input with lowercase letters, digits and _.',
    );
    expect(checkHttpTool(tool('t', { inputs: [input, input] }), [], []).inputs).toBe(
      'Give each input its own name.',
    );
  });
});

describe('hosts', () => {
  it('reads the host of an https URL only', () => {
    expect(hostOf(' https://Calendar.Example.com:8443/v1 ')).toBe('calendar.example.com');
    expect(hostOf('http://calendar.example.com')).toBe('');
  });

  it('allows exact hosts and subdomains of a wildcard, not its apex', () => {
    const allowed = ['calendar.example.com', '*.tools.test'];

    expect(hostAllowed('calendar.example.com', allowed)).toBe(true);
    expect(hostAllowed('crm.tools.test', allowed)).toBe(true);
    expect(hostAllowed('tools.test', allowed)).toBe(false);
    expect(hostAllowed('evil.example.com', allowed)).toBe(false);
  });
});

describe('versionState', () => {
  const saved = { status: 'saved', dirty: false } as const;
  const custom = { system: false, latestVersion: 2, unsavedChanges: false, problems: [] };

  it('says a new purpose has no version yet and offers version 1', () => {
    expect(versionState({ ...custom, latestVersion: null }, saved)).toEqual({
      label: 'Not saved as a version yet',
      next: 1,
      canSave: true,
      reason: null,
    });
  });

  it('waits while nothing changed, a change is saving, or problems remain', () => {
    expect(versionState(custom, saved)).toMatchObject({
      label: 'Version 2',
      canSave: false,
      reason: 'Nothing changed since version 2.',
    });
    expect(versionState(custom, { status: 'saving', dirty: true })).toMatchObject({
      label: 'Changes since version 2',
      next: 3,
      reason: 'Your last change is still being saved.',
    });
    const problem = { code: 'host_not_allowed', message: 'No.', field: 'tools.http[0].url' };
    const changed = { ...custom, unsavedChanges: true };
    expect(versionState({ ...changed, problems: [problem] }, saved).reason).toBe(
      'Fix 1 problem first.',
    );
    expect(versionState({ ...changed, problems: [problem, problem] }, saved).reason).toBe(
      'Fix 2 problems first.',
    );
  });

  it('never saves a built-in purpose', () => {
    expect(versionState({ ...custom, system: true }, saved)).toMatchObject({
      canSave: false,
      reason: 'Built-in purposes are read-only.',
    });
  });
});

describe('choices and sections', () => {
  it('labels classes, actions and sections', () => {
    expect(classLabel('telemarketing')).toBe('Outbound sales');
    expect(actionLabel('end_call')).toBe('End the call with a message');
    expect(SECTIONS.map(({ label }) => label)).toEqual([
      'Overview',
      'Flow',
      'Tools',
      'Guardrails',
      'Compliance',
    ]);
    expect(sectionLabel('guardrails')).toBe('Guardrails');
  });

  it('narrows a select value back to an option', () => {
    expect(choose(METHODS, 'PATCH')).toBe('PATCH');
    expect(choose(METHODS, 'TRACE')).toBeUndefined();
  });
});
