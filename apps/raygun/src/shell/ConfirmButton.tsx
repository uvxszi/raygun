import type { LucideIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';

/** How long the second tap is waited for. */
const ARMED_MS = 4000;

/**
 * An icon button for something that can't be undone (removing a folder, deleting a download): the
 * first tap turns it into a button saying what it does, the second does it. It goes back after a
 * few seconds or when focus moves away.
 */
export const ConfirmButton: FC<{
  /** The icon button's name: "Delete Big Buck Bunny". */
  label: string;
  /** What the second tap does: "Delete". */
  confirm: string;
  icon: LucideIcon;
  onConfirm: () => void;
  disabled?: boolean;
  className?: string;
  /** Replaces the icon button's own look (size, colour), for rows with smaller buttons. */
  idleClassName?: string;
  iconSize?: number;
}> = ({
  label,
  confirm,
  icon: Icon,
  onConfirm,
  disabled = false,
  className = '',
  idleClassName = 'text-muted hover:text-foreground size-10',
  iconSize = 18,
}) => {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), ARMED_MS);
    return () => clearTimeout(timer);
  }, [armed]);

  if (armed) {
    return (
      <button
        type="button"
        autoFocus
        disabled={disabled}
        onBlur={() => setArmed(false)}
        onClick={() => {
          setArmed(false);
          onConfirm();
        }}
        className="bg-primary text-background shrink-0 rounded-full px-3 py-1.5 text-xs font-semibold disabled:opacity-60"
      >
        {confirm}
      </button>
    );
  }
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={() => setArmed(true)}
      className={`${idleClassName} grid shrink-0 place-items-center rounded-full disabled:opacity-60 ${className}`}
    >
      <Icon aria-hidden size={iconSize} />
    </button>
  );
};
