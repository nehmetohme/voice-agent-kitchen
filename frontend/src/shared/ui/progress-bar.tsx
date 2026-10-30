import { Progress } from '@base-ui/react/progress';

interface ProgressBarProps {
  /** Names the bar for assistive technology and shows above it. */
  label: string;
  /** 0 to 100. */
  value: number;
}

/**
 * A labelled `role=progressbar` (Base UI Progress) with its percentage, for setup progress
 * (Zeigarnik). `aria-valuenow` carries the value.
 */
export function ProgressBar({ label, value }: ProgressBarProps) {
  const percent = Math.min(100, Math.max(0, Math.round(value)));
  return (
    <Progress.Root
      data-slot="progress"
      value={percent}
      getAriaValueText={() => `${percent} %`}
      className="grid grid-cols-2 gap-y-2"
    >
      <Progress.Label className="text-sm font-medium">{label}</Progress.Label>
      <span className="text-right text-sm text-muted">{percent} %</span>
      <Progress.Track className="col-span-2 h-2 overflow-hidden rounded-full bg-line">
        <Progress.Indicator className="rounded-full bg-success transition-all" />
      </Progress.Track>
    </Progress.Root>
  );
}
