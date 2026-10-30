import { useId, type ReactNode } from 'react';

interface EmptyStateProps {
  title: string;
  description: string;
  /** A decorative icon; give it `aria-hidden`. */
  icon?: ReactNode;
  /** The next step, usually one <Button> or link. */
  action?: ReactNode;
}

/** What a screen shows before it has content: what goes here and how to start. */
export function EmptyState({ title, description, icon, action }: EmptyStateProps) {
  const titleId = useId();
  return (
    <section
      aria-labelledby={titleId}
      className="flex max-w-readable flex-col items-start gap-4 rounded-md border border-line bg-surface p-8"
    >
      {icon}
      <div className="flex flex-col gap-2">
        <h2 id={titleId} className="text-lg font-semibold">
          {title}
        </h2>
        <p className="text-muted">{description}</p>
      </div>
      {action}
    </section>
  );
}
