// Contract voice-core v1 (media protocol, browser side): src/shared/voice/media-protocol.ts
// agrees with contracts/media/vak.media.v1.json and contracts/media/examples.json, the same
// files the server checks in backend/crates/providers/tests/contract_voice_core_media.rs.
//
// What it pins:
// - The schema compiles (JSON Schema 2020-12, Ajv) and the examples agree with it.
// - The client accepts what the server may send and nothing else: every valid server example
//   parses to exactly its JSON; every invalid one, unknown types and non-JSON text included,
//   parses to null.
// - The client knows every server event type in the schema (SERVER_EVENT_TYPES).
// - The client sends only what the schema allows: every client event it encodes is valid and
//   reads back unchanged.
// - The protocol constants (subprotocol, sample rates, frame length) match the schema.
//
// Pinned by the signed tag contract/voice-core/v1. Never edit this file; to change the contract,
// add a new version and get it signed.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
  type ClientEvent,
  encodeClientEvent,
  FRAME_MS,
  INPUT_SAMPLE_RATE,
  MEDIA_PROTOCOL,
  OUTPUT_SAMPLE_RATE,
  parseServerEvent,
  SERVER_EVENT_TYPES,
} from './media-protocol';

const contracts = join(import.meta.dirname, '../../../../contracts/media');
const examplesText = readFileSync(join(contracts, 'examples.json'), 'utf8');
const schemaText = readFileSync(join(contracts, 'vak.media.v1.json'), 'utf8');

// The shapes of the two contract files, as far as this test reads them.
const jsonObject = z.record(z.string(), z.unknown());
const side = z.object({
  valid: z.array(jsonObject),
  invalid: z.array(z.object({ why: z.string(), event: z.unknown() })),
});
const examples = z.object({ client: side, server: side }).parse(JSON.parse(examplesText));
const schemaJson: unknown = JSON.parse(schemaText);
const schema = z
  .object({ $id: z.string(), title: z.string(), $defs: z.record(z.string(), z.unknown()) })
  .parse(schemaJson);
const constOf = (value: unknown) => z.object({ const: z.unknown() }).parse(value).const;
const audioFormat = z.object({
  properties: z.object({ sample_rate: z.unknown(), frame_ms: z.unknown() }),
});

const ajv = new Ajv2020({ allErrors: true });
ajv.addSchema(z.record(z.string(), z.unknown()).parse(schemaJson));
const validator = (which: 'client' | 'server') =>
  ajv.compile({ $ref: `${schema.$id}#/$defs/${which}_event` });

/** The `type` const of every event in the schema's union for `which`. */
function schemaTypes(which: 'client' | 'server'): string[] {
  const union = z
    .object({ oneOf: z.array(z.object({ $ref: z.string() })) })
    .parse(schema.$defs[`${which}_event`]);
  return union.oneOf.map(({ $ref }) => {
    const def = z
      .object({ properties: z.object({ type: z.object({ const: z.string() }) }) })
      .parse(schema.$defs[$ref.replace('#/$defs/', '')]);
    return def.properties.type.const;
  });
}

const typesOf = (events: Record<string, unknown>[]) =>
  new Set(events.map((event) => event['type']));

/** The client event an example describes, built as the app would build it. */
function clientEvent(example: Record<string, unknown>): ClientEvent {
  switch (example['type']) {
    case 'session.start':
      return example['audio'] === undefined
        ? { type: 'session.start' }
        : { type: 'session.start', audio: { encoding: 'pcm_s16le', sample_rate: 16_000 } };
    case 'client.mute':
      return { type: 'client.mute', muted: example['muted'] === true };
    case 'session.end':
      return { type: 'session.end' };
    default:
      throw new Error(`no client event of type ${String(example['type'])}`);
  }
}

describe('vak.media.v1 contract v1 (browser)', () => {
  it('compiles the schema, and the examples agree with it', () => {
    for (const which of ['client', 'server'] as const) {
      const validate = validator(which);
      const rejected = examples[which].valid.filter((event) => !validate(event));
      const accepted = examples[which].invalid.filter(({ event }) => validate(event));
      expect(rejected).toStrictEqual([]);
      expect(accepted).toStrictEqual([]);
    }
  });

  it('parses every valid server event to exactly its JSON', () => {
    const parsed = examples.server.valid.map((event) => parseServerEvent(JSON.stringify(event)));
    expect(parsed).toStrictEqual(examples.server.valid);
  });

  it('refuses every invalid server event, and text that is not JSON', () => {
    const accepted = examples.server.invalid
      .filter(({ event }) => parseServerEvent(JSON.stringify(event)) !== null)
      .map(({ why }) => why);
    expect(accepted).toStrictEqual([]);
    expect(parseServerEvent('not json')).toBeNull();
    expect(parseServerEvent('')).toBeNull();
  });

  it('knows every server event type in the schema', () => {
    expect(SERVER_EVENT_TYPES.toSorted()).toStrictEqual(schemaTypes('server').toSorted());
    expect(typesOf(examples.server.valid)).toStrictEqual(new Set(schemaTypes('server')));
  });

  it('encodes only client events the schema allows, and they read back unchanged', () => {
    const validate = validator('client');
    expect(typesOf(examples.client.valid)).toStrictEqual(new Set(schemaTypes('client')));
    for (const example of examples.client.valid) {
      const sent: unknown = JSON.parse(encodeClientEvent(clientEvent(example)));
      expect(validate(sent)).toBe(true);
      expect(sent).toStrictEqual(example);
    }
  });

  it('uses the protocol constants of the schema', () => {
    expect(MEDIA_PROTOCOL).toBe('vak.media.v1');
    expect(schema.title.startsWith(`${MEDIA_PROTOCOL}:`)).toBe(true);
    const inputAudio = z
      .object({ properties: z.object({ sample_rate: z.unknown() }) })
      .parse(schema.$defs['input_audio']);
    expect(constOf(inputAudio.properties.sample_rate)).toBe(INPUT_SAMPLE_RATE);
    const started = z
      .object({
        properties: z.object({
          audio: z.object({ properties: z.object({ input: audioFormat, output: audioFormat }) }),
        }),
      })
      .parse(schema.$defs['session_started']);
    const { input, output } = started.properties.audio.properties;
    expect(constOf(input.properties.sample_rate)).toBe(INPUT_SAMPLE_RATE);
    expect(constOf(output.properties.sample_rate)).toBe(OUTPUT_SAMPLE_RATE);
    expect(constOf(input.properties.frame_ms)).toBe(FRAME_MS);
    expect(constOf(output.properties.frame_ms)).toBe(FRAME_MS);
    expect([INPUT_SAMPLE_RATE, OUTPUT_SAMPLE_RATE, FRAME_MS]).toStrictEqual([16_000, 24_000, 20]);
  });
});
