// A purpose's Tools, Guardrails and Compliance sections through the user's eyes, each held in
// a little stateful harness the way the purpose page holds its draft.
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';

import type {
  Baseline,
  ComplianceSettings,
  GuardrailSettings,
  HttpToolDraft,
  PurposeProblem,
  ToolCatalog,
  ToolSettings,
} from '../state/models';
import { ComplianceSection } from './compliance-section';
import { GuardrailsSection } from './guardrails-section';
import { ToolsSection } from './tools-section';

const CATALOG: ToolCatalog = {
  builtins: [
    {
      name: 'transfer_to_human',
      label: 'Transfer to a person',
      description: 'Hand over.',
      locked: false,
    },
    { name: 'mark_do_not_call', label: 'Opt out', description: 'Record an opt-out.', locked: true },
  ],
  allowedHosts: ['calendar.example.com', '*.tools.test'],
};

const BOOK_SLOT: HttpToolDraft = {
  name: 'book_slot',
  description: 'Books a slot.',
  method: 'POST',
  url: 'https://calendar.example.com/v1/slots',
  timeoutMs: 3000,
  secret: '',
  confirm: true,
  inputs: [],
};

const NO_PROBLEMS: PurposeProblem[] = [];

function Tools({
  initial,
  onChange,
  problems = NO_PROBLEMS,
  readOnly = false,
  catalog = CATALOG,
}: {
  initial: ToolSettings;
  onChange: (tools: ToolSettings) => void;
  problems?: PurposeProblem[];
  readOnly?: boolean;
  catalog?: ToolCatalog | undefined;
}) {
  const [tools, setTools] = useState(initial);
  return (
    <ToolsSection
      tools={tools}
      onChange={(next) => {
        setTools(next);
        onChange(next);
      }}
      catalog={catalog}
      readOnly={readOnly}
      problems={problems}
    />
  );
}

async function openDialog(trigger: string, title: string) {
  fireEvent.click(screen.getByRole('button', { name: trigger }));
  return screen.findByRole('dialog', { name: title });
}

describe('Tools section', () => {
  it('keeps the opt-out tool on and turns other built-ins on', () => {
    const onChange = vi.fn<(tools: ToolSettings) => void>();
    render(<Tools initial={{ builtins: ['mark_do_not_call'], http: [] }} onChange={onChange} />);

    const optOut = screen.getByRole('checkbox', { name: 'Opt out (always on)' });
    expect(optOut).toBeChecked();
    expect(optOut).toBeDisabled();
    fireEvent.click(screen.getByRole('checkbox', { name: 'Transfer to a person' }));
    expect(onChange).toHaveBeenLastCalledWith({
      builtins: ['mark_do_not_call', 'transfer_to_human'],
      http: [],
    });
    expect(screen.getByText(/They may call calendar.example.com, \*.tools.test/)).toBeVisible();
    expect(screen.getByText('No custom tools yet.')).toBeVisible();
  });

  it('adds an HTTP tool, checked before it is added, with its inputs', async () => {
    const onChange = vi.fn<(tools: ToolSettings) => void>();
    render(<Tools initial={{ builtins: ['mark_do_not_call'], http: [] }} onChange={onChange} />);

    const dialog = await openDialog('Add HTTP tool', 'Add HTTP tool');
    const field = (name: string) => within(dialog).getByRole('textbox', { name });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add tool' }));
    expect(field('Tool name')).toHaveAccessibleDescription(
      'What the LLM calls it, e.g. book_slot. Use lowercase letters, digits and _, starting with a letter.',
    );
    expect(within(dialog).getByText('Say what the tool does.')).toBeVisible();
    fireEvent.change(field('Tool name'), { target: { value: 'book_slot' } });
    fireEvent.change(field('What it does'), { target: { value: 'Books a slot.' } });
    fireEvent.change(field('URL'), { target: { value: 'https://evil.example.com/x' } });
    expect(field('URL')).toHaveAccessibleDescription(
      /evil.example.com isn't on the tool host allow-list/,
    );
    fireEvent.change(field('URL'), { target: { value: 'https://calendar.example.com/v1/slots' } });
    fireEvent.change(within(dialog).getByRole('combobox', { name: 'Method' }), {
      target: { value: 'PATCH' },
    });
    fireEvent.change(within(dialog).getByRole('spinbutton', { name: 'Timeout (ms)' }), {
      target: { value: '5000' },
    });
    fireEvent.change(field('Secret name'), { target: { value: 'CALENDAR_API_KEY' } });
    fireEvent.click(
      within(dialog).getByRole('checkbox', { name: 'Confirm with the caller first' }),
    );
    expect(within(dialog).getByText('No inputs: the tool is called without any.')).toBeVisible();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add input' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add input' }));
    const first = within(dialog).getByRole('group', { name: 'Input 1' });
    fireEvent.change(within(first).getByRole('textbox', { name: 'Name' }), {
      target: { value: 'people' },
    });
    fireEvent.change(within(first).getByRole('combobox', { name: 'Type' }), {
      target: { value: 'integer' },
    });
    fireEvent.change(within(first).getByRole('textbox', { name: 'Description' }), {
      target: { value: 'How many.' },
    });
    fireEvent.click(within(first).getByRole('checkbox', { name: 'Required' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add tool' }));
    expect(
      within(dialog).getByText('Name each input with lowercase letters, digits and _.'),
    ).toBeVisible();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Remove input 2' }));
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add tool' }));

    await waitFor(() => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    });
    expect(onChange).toHaveBeenLastCalledWith({
      builtins: ['mark_do_not_call'],
      http: [
        {
          name: 'book_slot',
          description: 'Books a slot.',
          method: 'PATCH',
          url: 'https://calendar.example.com/v1/slots',
          timeoutMs: 5000,
          secret: 'CALENDAR_API_KEY',
          confirm: false,
          inputs: [{ name: 'people', type: 'integer', description: 'How many.', required: false }],
        },
      ],
    });
    const card = screen.getByRole('group', { name: 'book_slot' });
    expect(card).toHaveTextContent('PATCH https://calendar.example.com/v1/slots');
    expect(card).toHaveTextContent('Inputs: people · Secret: CALENDAR_API_KEY');
  });

  it('edits and removes a tool, and shows its problem', async () => {
    const onChange = vi.fn<(tools: ToolSettings) => void>();
    const problem = {
      code: 'host_not_allowed',
      message: 'This host is not allowed.',
      field: 'tools.http[0].url',
    };
    render(
      <Tools
        initial={{ builtins: ['mark_do_not_call'], http: [BOOK_SLOT] }}
        onChange={onChange}
        problems={[problem]}
      />,
    );
    const card = screen.getByRole('group', { name: 'book_slot' });
    expect(card).toHaveTextContent('No inputs · Confirms first');
    expect(within(card).getByText('This host is not allowed.')).toBeVisible();

    const dialog = await openDialog('Edit book_slot', 'Edit book_slot');
    const description = within(dialog).getByRole('textbox', { name: 'What it does' });
    expect(description).toHaveValue('Books a slot.');
    fireEvent.change(description, { target: { value: 'Books a visit.' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Save tool' }));
    await waitFor(() => {
      expect(screen.getByRole('group', { name: 'book_slot' })).toHaveTextContent('Books a visit.');
    });
    expect(onChange).toHaveBeenLastCalledWith({
      builtins: ['mark_do_not_call'],
      http: [{ ...BOOK_SLOT, description: 'Books a visit.' }],
    });

    fireEvent.click(
      within(screen.getByRole('group', { name: 'book_slot' })).getByRole('button', {
        name: 'Remove',
      }),
    );
    const confirm = await screen.findByRole('alertdialog', { name: 'Remove book_slot?' });
    fireEvent.click(within(confirm).getByRole('button', { name: 'Remove' }));
    expect(await screen.findByText('No custom tools yet.')).toBeVisible();
  });

  it('shows tools without actions when read-only, and says when no host is allowed', () => {
    render(
      <Tools
        initial={{ builtins: ['mark_do_not_call'], http: [BOOK_SLOT] }}
        onChange={vi.fn<(tools: ToolSettings) => void>()}
        readOnly
        catalog={{ builtins: CATALOG.builtins, allowedHosts: [] }}
      />,
    );

    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    expect(screen.getByText(/No host is allowed yet/)).toBeVisible();
    expect(screen.getByRole('group', { name: 'book_slot' })).toBeVisible();
  });
});

const GUARDRAILS: GuardrailSettings = {
  allowedTopics: [],
  never: [],
  escalations: [],
  maxCallMinutes: 30,
  neverAsk: [],
};

function Guardrails({
  onChange,
  readOnly = false,
  initial = GUARDRAILS,
}: {
  onChange: (guardrails: GuardrailSettings) => void;
  readOnly?: boolean;
  initial?: GuardrailSettings;
}) {
  const [guardrails, setGuardrails] = useState(initial);
  return (
    <GuardrailsSection
      guardrails={guardrails}
      onChange={(next) => {
        setGuardrails(next);
        onChange(next);
      }}
      packClaims={['Saying it is a human.']}
      readOnly={readOnly}
    />
  );
}

describe('Guardrails section', () => {
  it('edits topics, never-rules and limits, beside the rule pack claims', () => {
    const onChange = vi.fn<(guardrails: GuardrailSettings) => void>();
    render(<Guardrails onChange={onChange} />);

    expect(screen.getByRole('list', { name: 'From the rule pack' })).toHaveTextContent(
      'Saying it is a human.',
    );
    const topic = screen.getByRole('textbox', { name: 'New topic' });
    fireEvent.change(topic, { target: { value: 'Appointments' } });
    fireEvent.keyDown(topic, { key: 'Enter' });
    const never = screen.getByRole('textbox', { name: 'New never-rule' });
    fireEvent.change(never, { target: { value: 'Promise refunds.' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add never-rule' }));
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Longest call (minutes)' }), {
      target: { value: '10' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: 'Card numbers' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Health details' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Card numbers' }));

    expect(onChange).toHaveBeenLastCalledWith({
      allowedTopics: ['Appointments'],
      never: ['Promise refunds.'],
      escalations: [],
      maxCallMinutes: 10,
      neverAsk: ['health'],
    });
  });

  it('adds an escalation rule, with a message to end the call, and removes it', async () => {
    const onChange = vi.fn<(guardrails: GuardrailSettings) => void>();
    render(<Guardrails onChange={onChange} />);
    expect(screen.getByText('No escalations yet.')).toBeVisible();

    const dialog = await openDialog('Add escalation rule', 'Add escalation rule');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add rule' }));
    expect(within(dialog).getByText('Ask a yes/no question.')).toBeVisible();
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'When' }), {
      target: { value: ' Is it an emergency? ' },
    });
    fireEvent.change(within(dialog).getByRole('combobox', { name: 'Then' }), {
      target: { value: 'end_call' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add rule' }));
    expect(
      within(dialog).getByText('Say what the agent says before it ends the call.'),
    ).toBeVisible();
    fireEvent.change(within(dialog).getByRole('textbox', { name: 'Message' }), {
      target: { value: 'Please call 911.' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Add rule' }));

    const rules = await screen.findByRole('list', { name: 'Escalation rules' });
    expect(rules).toHaveTextContent('Is it an emergency? → End the call with a message');
    expect(onChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        escalations: [
          { when: 'Is it an emergency?', action: 'end_call', message: 'Please call 911.' },
        ],
      }),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Remove Is it an emergency?' }));
    expect(screen.getByText('No escalations yet.')).toBeVisible();
  });

  it('shows the rules without actions when read-only', () => {
    render(
      <Guardrails
        onChange={vi.fn<(guardrails: GuardrailSettings) => void>()}
        readOnly
        initial={{
          ...GUARDRAILS,
          escalations: [{ when: 'Is the caller upset?', action: 'flag', message: '' }],
        }}
      />,
    );

    expect(screen.getByRole('list', { name: 'Escalation rules' })).toHaveTextContent(
      'Is the caller upset? → Flag the call for review',
    );
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: 'Longest call (minutes)' })).toBeDisabled();
  });
});

function baseline(jurisdiction: string, recordingsDays: number): Baseline {
  const retention = { recordingsDays, transcriptsDays: 365, auditDays: 1826 };
  return {
    jurisdiction,
    rulePack: `${jurisdiction.toLowerCase()}-booking v1`,
    lockedTexts: ["Hello, you're speaking with an AI assistant."],
    optOutPhrases: ['stop calling me'],
    optOutConfirmation: "Understood. We won't contact you again.",
    forbiddenClaims: ['Saying it is a human.'],
    retention,
    effective: { optOutPhrases: ['stop calling me'], forbiddenClaims: [], retention },
  };
}

const COMPLIANCE: ComplianceSettings = {
  optOutPhrases: [],
  forbiddenClaims: [],
  recordingsDays: null,
  transcriptsDays: 30,
};

function Compliance({
  onChange,
  readOnly = false,
}: {
  onChange: (compliance: ComplianceSettings) => void;
  readOnly?: boolean;
}) {
  const [compliance, setCompliance] = useState(COMPLIANCE);
  return (
    <ComplianceSection
      compliance={compliance}
      onChange={(next) => {
        setCompliance(next);
        onChange(next);
      }}
      baselines={[baseline('US', 180), baseline('US-CA', 90)]}
      readOnly={readOnly}
    />
  );
}

describe('Compliance section', () => {
  it('shows each jurisdiction pack baseline read-only', () => {
    render(<Compliance onChange={vi.fn<(compliance: ComplianceSettings) => void>()} />);

    const pack = screen.getByRole('region', { name: 'Rule pack baseline' });
    expect(pack).toHaveTextContent('Rule pack us-booking v1: always applies');
    expect(pack).toHaveTextContent(
      'Kept: recordings 180 days, transcripts 365 days, audit 1826 days.',
    );
    fireEvent.change(screen.getByRole('combobox', { name: 'Jurisdiction' }), {
      target: { value: 'US-CA' },
    });
    expect(pack).toHaveTextContent('Rule pack us-ca-booking v1: always applies');
    expect(pack).toHaveTextContent('With this purpose: recordings 90 days');
  });

  it('adds phrases and claims, and caps retention', () => {
    const onChange = vi.fn<(compliance: ComplianceSettings) => void>();
    render(<Compliance onChange={onChange} />);

    const phrase = screen.getByRole('textbox', { name: 'New opt-out phrase' });
    fireEvent.change(phrase, { target: { value: 'take me off' } });
    fireEvent.keyDown(phrase, { key: 'Enter' });
    const claim = screen.getByRole('textbox', { name: 'New forbidden claim' });
    fireEvent.change(claim, { target: { value: 'Guaranteed slots.' } });
    fireEvent.keyDown(claim, { key: 'Enter' });
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Recordings' }), {
      target: { value: '30' },
    });
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Transcripts' }), {
      target: { value: '' },
    });

    expect(onChange).toHaveBeenLastCalledWith({
      optOutPhrases: ['take me off'],
      forbiddenClaims: ['Guaranteed slots.'],
      recordingsDays: 30,
      transcriptsDays: null,
    });
  });

  it('changes nothing when read-only', () => {
    render(<Compliance onChange={vi.fn<(compliance: ComplianceSettings) => void>()} readOnly />);

    expect(screen.queryByRole('textbox', { name: 'New opt-out phrase' })).not.toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: 'Recordings' })).toBeDisabled();
    expect(screen.getByRole('spinbutton', { name: 'Transcripts' })).toHaveValue(30);
  });
});
