import { LoaderCircle, Trash2 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

/**
 * Why an extension, plugin or addon does nothing in Raygun ("Not for Raygun: Adds a mini player
 * to Nuclear; Raygun has its own"), as the line under its name.
 */
export const NotForRaygunNote: FC<{ reason: string; className?: string }> = ({
  reason,
  className = '',
}) => {
  const { t } = useTranslation();
  return (
    <span className={`text-muted text-xs ${className}`}>
      {t('extensions.notForRaygunWhy', { reason })}
    </span>
  );
};

/**
 * Remove, in place of the on/off switch of one on this device that does nothing in Raygun (an
 * extension installed before, an addon added before).
 */
export const RemoveUnused: FC<{
  name: string;
  busy?: boolean;
  onRemove: () => void;
  /** Its background: the row's own is the surface's in some lists. */
  tone?: string;
}> = ({ name, busy = false, onRemove, tone = 'bg-surface' }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t('extensions.removeNamed', { name })}
      disabled={busy}
      onClick={onRemove}
      className={`${tone} hover:text-foreground flex shrink-0 items-center gap-1.5 rounded-full px-3 py-1.5 text-sm disabled:opacity-60`}
    >
      {busy ? (
        <LoaderCircle aria-hidden size={14} className="animate-spin" />
      ) : (
        <Trash2 aria-hidden size={14} />
      )}
      {t('extensions.remove')}
    </button>
  );
};
