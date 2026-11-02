import { useLayoutEffect, useMemo, useRef, useState, type RefObject } from 'react';

import { columnIndex, columnsOf } from '../state/layout';
import type { Path } from '../state/models';
import type { PlanEditor } from '../state/use-plan-editor';
import { StepCard } from './step-card';

interface Line {
  id: string;
  d: string;
  broken: boolean;
}

/** Where a card sits inside the canvas. */
interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

function boxesIn(canvas: HTMLElement): Map<string, Box> {
  const origin = canvas.getBoundingClientRect();
  const boxes = new Map<string, Box>();
  for (const card of canvas.querySelectorAll<HTMLElement>('[data-step-id]')) {
    const rect = card.getBoundingClientRect();
    const left = rect.left - origin.left + canvas.scrollLeft;
    const top = rect.top - origin.top + canvas.scrollTop;
    boxes.set(card.dataset['stepId'] ?? '', {
      left,
      top,
      right: left + rect.width,
      bottom: top + rect.height,
    });
  }
  return boxes;
}

/**
 * A path's line: forward paths go from the right edge of one card to the left edge of the
 * next; paths back (or within a column) loop under both cards, so they never cross a card.
 */
function lineOf(path: Path, boxes: Map<string, Box>, forward: boolean): string | null {
  const from = boxes.get(path.from);
  const to = boxes.get(path.to);
  if (from === undefined || to === undefined) return null;
  if (forward) {
    const [x1, y1] = [from.right, (from.top + from.bottom) / 2];
    const [x2, y2] = [to.left, (to.top + to.bottom) / 2];
    const bend = (x2 - x1) / 2;
    return `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`;
  }
  const [x1, y1] = [(from.left + from.right) / 2, from.bottom];
  const [x2, y2] = [(to.left + to.right) / 2, to.bottom];
  const low = Math.max(y1, y2) + 32;
  return `M ${x1} ${y1} C ${x1} ${low}, ${x2} ${low}, ${x2} ${y2}`;
}

/** Measures the cards after each layout and when the canvas resizes. */
function useLines(canvas: RefObject<HTMLDivElement | null>, editor: PlanEditor): Line[] {
  const [lines, setLines] = useState<Line[]>([]);
  const { draft, view } = editor;
  const columns = useMemo(() => columnIndex(columnsOf(draft)), [draft]);
  useLayoutEffect(() => {
    const element = canvas.current;
    if (element === null) return undefined;
    const broken = new Set(view.problems.map((problem) => problem.pathId));
    const measure = () => {
      const boxes = boxesIn(element);
      setLines(
        draft.paths.flatMap((path) => {
          const forward = (columns.get(path.to) ?? 0) > (columns.get(path.from) ?? 0);
          const d = lineOf(path, boxes, forward);
          return d === null ? [] : [{ id: path.id, d, broken: broken.has(path.id) }];
        }),
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, [canvas, columns, draft, view]);
  return lines;
}

/**
 * The flow: steps in columns, left to right from the start, joined by drawn paths (UC-3).
 * The drawing is decoration (`aria-hidden`); every path is also listed, in words, on the step
 * it leaves, which is where keyboard and screen-reader users work with it.
 */
export function PlanCanvas({ editor }: { editor: PlanEditor }) {
  const canvas = useRef<HTMLDivElement>(null);
  const lines = useLines(canvas, editor);
  const columns = columnsOf(editor.draft);
  return (
    <div ref={canvas} className="relative overflow-x-auto pb-12">
      <svg aria-hidden className="pointer-events-none absolute inset-0 size-full overflow-visible">
        {lines.map((line) => (
          <path
            key={line.id}
            d={line.d}
            fill="none"
            strokeWidth={2}
            strokeDasharray={line.broken ? '6 4' : undefined}
            className={line.broken ? 'stroke-danger' : 'stroke-muted'}
          />
        ))}
      </svg>
      <div className="relative flex items-start gap-16">
        {columns.map((column) => (
          <div key={column[0]?.id ?? 'empty'} className="flex flex-col gap-8">
            {column.map((step) => (
              <StepCard key={step.id} step={step} editor={editor} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
