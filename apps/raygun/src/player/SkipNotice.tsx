import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem } from '../collections/useCollections';
import { useSourcePickerStore } from '../music/sourcePickerStore';
import { usePlayerSelect, useQueueSelect } from './usePlayback';

/** How long a skip's reason stays under the player bar. */
const NOTICE_MS = 8_000;

/** A skip as the notice shows it: why, and which queue item nothing played. */
type Skip = { reason: string; itemId: string | null };

/**
 * Why a song was skipped (M46.48): playback moves on when a song can't play (after looking for
 * another source for it), and the reason stays here for a while after the next song starts,
 * with Find a source… for the song that was skipped.
 */
export const SkipNotice: FC = () => {
  const { t } = useTranslation();
  const error = usePlayerSelect((state) => state?.error ?? null);
  const skippedId = usePlayerSelect((state) => state?.skippedItemId ?? null);
  const [seen, setSeen] = useState<Skip | null>(null);
  const [hidden, setHidden] = useState<Skip | null>(null);
  // A new reason shows at once (state adjusted while rendering, not in an effect).
  if (error && error !== seen?.reason) setSeen({ reason: error, itemId: skippedId });
  const skipped = useQueueSelect((queue) =>
    seen?.itemId ? queue?.items.find((item) => item.id === seen.itemId) : undefined,
  );
  useEffect(() => {
    if (!seen) return undefined;
    const timer = window.setTimeout(() => setHidden(seen), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [seen]);
  if (!seen || hidden === seen) return null;
  return (
    <p
      role="status"
      data-testid="skip-notice"
      className="text-muted flex min-w-0 items-center gap-2 px-4 pb-2 text-xs"
    >
      <span className="min-w-0 truncate">{t('player.skipped', { reason: seen.reason })}</span>
      {skipped && (
        <button
          type="button"
          onClick={() => {
            useSourcePickerStore.getState().open(fromQueueItem(skipped));
            setHidden(seen);
          }}
          className="text-foreground shrink-0 font-medium hover:underline"
        >
          {t('sourcePicker.menu')}
        </button>
      )}
    </p>
  );
};
