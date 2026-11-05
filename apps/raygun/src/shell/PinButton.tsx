import { Pin, PinOff } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useIsDesktop } from './useIsDesktop';
import type { PinTarget } from './useRail';
import { useRail } from './useRail';

/**
 * Pin to / Unpin from quick access (M46.29), in a page's top bar: pins stay on top of the sidebar.
 * A page of the audio library is listed anyway, so unpinned it moves back among the rest. The
 * sidebar is the desktop's, so phones don't show it (their drawer's menus pin).
 */
export const PinButton: FC<{ target: PinTarget | null }> = ({ target }) => {
  const { t } = useTranslation();
  const desktop = useIsDesktop();
  const rail = useRail();
  if (!desktop || !target) return null;
  const pinned = rail.isPinned(target);
  return (
    <button
      type="button"
      aria-label={t(pinned ? 'rail.unpin' : 'rail.pin')}
      data-hint={t(pinned ? 'rail.unpin' : 'rail.pin')}
      aria-pressed={pinned}
      onClick={() => (pinned ? rail.unpin(target) : rail.pin(target))}
      className={`hover:bg-surface grid size-11 place-items-center rounded-full ${pinned ? 'text-primary' : ''}`}
    >
      {pinned ? <PinOff aria-hidden size={20} /> : <Pin aria-hidden size={20} />}
    </button>
  );
};
