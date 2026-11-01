import { Link } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import type { FC } from 'react';

type Action = {
  label: string;
  icon?: LucideIcon;
  to?: string;
  search?: Record<string, unknown>;
  onClick?: () => void;
};

/**
 * What a screen shows when it has nothing yet (Mihon's): a large icon, a line saying why, and
 * the one thing to do about it.
 */
export const EmptyState: FC<{
  icon: LucideIcon;
  title: string;
  body?: string;
  action?: Action;
}> = ({ icon: Icon, title, body, action }) => {
  const button =
    'bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold';
  return (
    <div className="flex flex-col items-center gap-3 px-8 py-16 text-center">
      <span className="bg-primary/10 text-primary mb-1 grid size-20 place-items-center rounded-full">
        <Icon aria-hidden size={36} />
      </span>
      <p className="text-base font-semibold">{title}</p>
      {body && <p className="text-muted max-w-xs text-sm">{body}</p>}
      {action &&
        (action.to ? (
          <Link to={action.to} search={action.search} className={`${button} mt-2`}>
            {action.icon && <action.icon aria-hidden size={16} />}
            {action.label}
          </Link>
        ) : (
          <button type="button" onClick={action.onClick} className={`${button} mt-2`}>
            {action.icon && <action.icon aria-hidden size={16} />}
            {action.label}
          </button>
        ))}
    </div>
  );
};

/** A centred spinner while a screen's content loads. */
export const Loading: FC<{ label: string }> = ({ label }) => (
  <div className="grid place-items-center py-16">
    <LoaderCircle
      role="status"
      aria-label={label}
      size={28}
      className="text-primary animate-spin"
    />
  </div>
);
