import { Progress } from '@base-ui/react/progress';

interface ProgressBarProps {
  /** Names the bar for assistive technology; shown above it unless `compact`. */
  label: string;
  /** 0 to 100. */
  value: number;
  /** One line for a tight header: the percentage, then a slim bar. The label is not shown. */
  compact?: boolean | undefined;
}

/**
 * A labelled `role=progressbar` (Base UI Progress) with its percentage, for setup progress
 * (Zeigarnik). `aria-valuenow` carries the value.
 */
export function ProgressBar({ label, value, compact = false }: ProgressBarProps) {
  const percent = Math.min(100, Math.max(0, Math.round(value)));
  return (
    <Progress.Root
      data-slot="progress"
      value={percent}
      getAriaValueText={() => `${percent} %`}
      className={compact ? 'flex items-center gap-2' : 'grid grid-cols-2 gap-y-2'}
    >
      <Progress.Label className={compact ? 'sr-only' : 'text-sm font-medium'}>
        {label}
      </Progress.Label>
      <span
        className={
          compact
            ? 'text-sm font-medium whitespace-nowrap tabular-nums'
            : 'text-right text-sm text-muted'
        }
      >
        {percent} %
      </span>
      <Progress.Track
        className={
          compact
            ? 'h-2 w-16 overflow-hidden rounded-full bg-line'
            : 'col-span-2 h-2 overflow-hidden rounded-full bg-line'
        }
      >
        <Progress.Indicator className="rounded-full bg-success transition-all" />
      </Progress.Track>
    </Progress.Root>
  );
}
