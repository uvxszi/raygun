import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { usePlaybackState } from './usePlayback';

/** How long a skip's reason stays under the player bar. */
const NOTICE_MS = 8_000;

/**
 * Why a song was skipped (M46.48): playback moves on when a song can't play, and the reason the
 * resolver or the engine gave stays here for a while after the next song starts.
 */
export const SkipNotice: FC = () => {
  const { t } = useTranslation();
  const error = usePlaybackState().data?.error ?? null;
  const [seen, setSeen] = useState<string | null>(null);
  const [hidden, setHidden] = useState<string | null>(null);
  // A new reason shows at once (state adjusted while rendering, not in an effect).
  if (error && error !== seen) setSeen(error);
  useEffect(() => {
    if (!seen) return undefined;
    const timer = window.setTimeout(() => setHidden(seen), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [seen]);
  if (!seen || hidden === seen) return null;
  return (
    <p role="status" data-testid="skip-notice" className="text-muted truncate px-4 pb-2 text-xs">
      {t('player.skipped', { reason: seen })}
    </p>
  );
};
