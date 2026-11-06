import { ArrowDown } from 'lucide-react';
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

import { Button } from './button';

/** How close to the end still counts as being at the newest content, in CSS px. */
const AT_END_PX = 16;

function isAtEnd(element: HTMLElement): boolean {
  return element.scrollHeight - element.scrollTop - element.clientHeight <= AT_END_PX;
}

interface FollowScrollProps {
  /** Names the scroll region (a `section`, so `role=region`). */
  label: string;
  /** Follow the newest content (default). Off, it is a plain scroll region from the start. */
  follow?: boolean | undefined;
  children: ReactNode;
}

/**
 * A region that scrolls on its own and follows its newest content, like a chat: while the
 * reader is at the end, every new line scrolls into view. Scrolled up, it stops following and
 * offers "Jump to latest"; back at the end, by scrolling or the button, it follows again. The
 * region takes focus, so the keyboard can scroll it. Give it a bounded height (a flex parent).
 * With `follow` off (content that doesn't grow, such as a finished transcript), it opens at the
 * start and never moves by itself.
 */
export function FollowScroll({ label, follow = true, children }: FollowScrollProps) {
  const region = useRef<HTMLElement>(null);
  const following = useRef(follow);
  const [behind, setBehind] = useState(false);

  const toEnd = useCallback(() => {
    const element = region.current;
    if (element !== null) element.scrollTop = element.scrollHeight;
  }, []);

  // Every render may add content (a line, a longer partial): stay at the end while following.
  useLayoutEffect(() => {
    if (following.current) toEnd();
  });

  // Size changes without a render (a resized window, wrapped text) keep the end in view too.
  useEffect(() => {
    const element = region.current;
    if (element === null || typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(() => {
      if (following.current) toEnd();
    });
    observer.observe(element);
    for (const child of element.children) observer.observe(child);
    return () => {
      observer.disconnect();
    };
  }, [toEnd]);

  const onScroll = () => {
    const element = region.current;
    if (element === null || !follow) return;
    following.current = isAtEnd(element);
    setBehind(!following.current);
  };

  const jump = () => {
    following.current = true;
    setBehind(false);
    toEnd();
    region.current?.focus();
  };

  return (
    <div data-slot="follow-scroll" className="relative flex min-h-0 flex-1 flex-col">
      <section
        ref={region}
        aria-label={label}
        // Scrollable regions must be reachable by keyboard (WCAG 2.1.1).
        // oxlint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        onScroll={onScroll}
        className="min-h-0 flex-1 overflow-y-auto rounded-md focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
      >
        <div className="flex min-h-full flex-col">{children}</div>
      </section>
      {behind && (
        <div className="pointer-events-none absolute inset-x-0 bottom-3 flex justify-center">
          <Button className="pointer-events-auto shadow-md" onClick={jump}>
            <ArrowDown aria-hidden className="size-4" />
            Jump to latest
          </Button>
        </div>
      )}
    </div>
  );
}
