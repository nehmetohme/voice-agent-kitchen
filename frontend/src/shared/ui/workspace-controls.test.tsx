// The workspace primitives through the user's eyes: the compact progress bar, the controlled
// confirmation, the "More actions" menu, the details popover and the following scroll region.
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';

import { ActionMenu } from './action-menu';
import { ConfirmDialog } from './confirm-dialog';
import { DetailsPopover } from './details-popover';
import { FollowScroll } from './follow-scroll';
import { ProgressBar } from './progress-bar';

describe('ProgressBar, compact', () => {
  it('keeps its name for assistive technology and shows only the percentage', () => {
    render(<ProgressBar compact label="Setup progress" value={50} />);

    const bar = screen.getByRole('progressbar', { name: 'Setup progress' });
    expect(bar).toHaveAttribute('aria-valuenow', '50');
    expect(screen.getByText('50 %')).toBeVisible();
    expect(screen.getByText('Setup progress')).toHaveClass('sr-only');
  });
});

/** A menu item that opens the confirmation, as the agent's More actions do. */
function Opener({ onConfirm }: { onConfirm: () => void }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <ActionMenu
        label="More actions"
        actions={[
          {
            label: 'Archive agent…',
            danger: true,
            onSelect: () => {
              setOpen(true);
            },
          },
        ]}
      />
      <ConfirmDialog
        open={open}
        onOpenChange={setOpen}
        title="Archive Front desk?"
        description="It leaves the list."
        confirmLabel="Archive"
        onConfirm={onConfirm}
      />
    </>
  );
}

describe('ConfirmDialog, opened from elsewhere', () => {
  it('has no trigger of its own, opens from a menu item and closes on Cancel', async () => {
    const onConfirm = vi.fn<() => void>();
    render(<Opener onConfirm={onConfirm} />);
    expect(screen.queryByRole('button', { name: 'Archive agent' })).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    const item = await screen.findByRole('menuitem', { name: 'Archive agent…' });
    expect(item).toHaveClass('text-danger');
    fireEvent.click(item);
    const dialog = await screen.findByRole('alertdialog', { name: 'Archive Front desk?' });
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    await waitFor(() => {
      expect(dialog).not.toBeInTheDocument();
    });
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

describe('ActionMenu', () => {
  it('lists its actions as 44 px menu items and runs the chosen one', async () => {
    const onSelect = vi.fn<() => void>();
    render(<ActionMenu label="More actions" actions={[{ label: 'Duplicate', onSelect }]} />);

    const trigger = screen.getByRole('button', { name: 'More actions' });
    expect(trigger).toHaveAttribute('data-variant', 'ghost');
    fireEvent.click(trigger);
    const item = await screen.findByRole('menuitem', { name: 'Duplicate' });
    expect(item).toHaveClass('min-h-target', 'text-ink');
    fireEvent.click(item);

    expect(onSelect).toHaveBeenCalledOnce();
  });
});

describe('DetailsPopover', () => {
  it('opens a dialog named by its title, from a button that says what it details', async () => {
    render(
      <DetailsPopover triggerLabel="Details" title="Latency">
        <p>Every test call since the server started.</p>
      </DetailsPopover>,
    );
    expect(screen.queryByText('Every test call since the server started.')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: 'Latency details' }));

    const popover = await screen.findByRole('dialog', { name: 'Latency' });
    expect(popover).toHaveTextContent('Every test call since the server started.');
  });
});

/** jsdom has no layout: give the region a size and a scroll position by hand. */
function layOut(region: HTMLElement, sizes: { scrollHeight: number; clientHeight: number }) {
  Object.defineProperty(region, 'scrollHeight', { configurable: true, value: sizes.scrollHeight });
  Object.defineProperty(region, 'clientHeight', { configurable: true, value: sizes.clientHeight });
}

function Lines({ count, follow = true }: { count: number; follow?: boolean }) {
  return (
    <FollowScroll label="Transcript" follow={follow}>
      <ol>
        {Array.from({ length: count }, (_, index) => (
          <li key={index}>Line {index + 1}</li>
        ))}
      </ol>
    </FollowScroll>
  );
}

describe('FollowScroll', () => {
  it('is a named, focusable region that follows new content while at the end', () => {
    const { rerender } = render(<Lines count={2} />);
    const region = screen.getByRole('region', { name: 'Transcript' });
    expect(region).toHaveAttribute('tabindex', '0');

    layOut(region, { scrollHeight: 900, clientHeight: 300 });
    rerender(<Lines count={3} />);

    expect(region.scrollTop).toBe(900);
    expect(screen.queryByRole('button', { name: 'Jump to latest' })).toBeNull();
  });

  it('stops following when scrolled up, and jumps back on request', () => {
    const { rerender } = render(<Lines count={2} />);
    const region = screen.getByRole('region', { name: 'Transcript' });
    layOut(region, { scrollHeight: 900, clientHeight: 300 });

    region.scrollTop = 100;
    fireEvent.scroll(region);
    layOut(region, { scrollHeight: 1_000, clientHeight: 300 });
    rerender(<Lines count={3} />);
    expect(region.scrollTop).toBe(100);

    fireEvent.click(screen.getByRole('button', { name: 'Jump to latest' }));
    expect(region.scrollTop).toBe(1_000);
    expect(region).toHaveFocus();
    expect(screen.queryByRole('button', { name: 'Jump to latest' })).toBeNull();
  });

  it('follows again once scrolled back to the end', () => {
    const { rerender } = render(<Lines count={2} />);
    const region = screen.getByRole('region', { name: 'Transcript' });
    layOut(region, { scrollHeight: 900, clientHeight: 300 });

    region.scrollTop = 0;
    fireEvent.scroll(region);
    expect(screen.getByRole('button', { name: 'Jump to latest' })).toBeVisible();
    region.scrollTop = 590;
    fireEvent.scroll(region);
    expect(screen.queryByRole('button', { name: 'Jump to latest' })).toBeNull();

    layOut(region, { scrollHeight: 1_200, clientHeight: 300 });
    act(() => {
      rerender(<Lines count={4} />);
    });
    expect(region.scrollTop).toBe(1_200);
  });

  it('stays where the reader is, from the start, when it does not follow', () => {
    const { rerender } = render(<Lines count={2} follow={false} />);
    const region = screen.getByRole('region', { name: 'Transcript' });
    layOut(region, { scrollHeight: 900, clientHeight: 300 });
    rerender(<Lines count={3} follow={false} />);
    expect(region.scrollTop).toBe(0);

    region.scrollTop = 100;
    fireEvent.scroll(region);
    expect(screen.queryByRole('button', { name: 'Jump to latest' })).toBeNull();
  });
});
