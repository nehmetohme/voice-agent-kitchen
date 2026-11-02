import {
  CircleHelp,
  ClipboardList,
  MessageSquare,
  PhoneForwarded,
  PhoneOff,
  Play,
  Wrench,
  type LucideIcon,
} from 'lucide-react';

import type { StepKind } from '../state/models';

/** How each kind of step reads on the canvas, with one icon set (SIM-3). */
export const STEP_KINDS: Record<StepKind, { label: string; Icon: LucideIcon }> = {
  start: { label: 'Start', Icon: Play },
  say: { label: 'Say', Icon: MessageSquare },
  ask: { label: 'Ask', Icon: CircleHelp },
  collect: { label: 'Collect', Icon: ClipboardList },
  tool: { label: 'Tool', Icon: Wrench },
  transfer: { label: 'Transfer', Icon: PhoneForwarded },
  end: { label: 'End', Icon: PhoneOff },
};

/** The kinds the operator can add: one start step per plan, so not `start` (Hick: six). */
export const ADDABLE_KINDS: readonly StepKind[] = [
  'say',
  'ask',
  'collect',
  'tool',
  'transfer',
  'end',
];
