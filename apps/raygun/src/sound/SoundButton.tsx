import { Link } from '@tanstack/react-router';
import { Scissors, Settings2, SlidersVertical } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useNowPlaying } from '../player/usePlayback';
import { Sheet } from '../shell/Sheet';
import { EqualizerPanel } from './EqualizerPanel';
import { useSongEditStore } from './songEditStore';
import { useSoundPrefs } from './useSound';

/**
 * The equalizer from the player bar and the full player (M46.12), with the playing song's own
 * sound (trims, volume, lyrics) and the rest of Settings → Sound a click away.
 */
export const SoundButton: FC<{ className?: string }> = ({ className = '' }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const { sound, change } = useSoundPrefs();
  const { item } = useNowPlaying();
  const openEditor = useSongEditStore((store) => store.open);
  const shaped = sound.eqEnabled || sound.bassBoost > 0 || sound.reverb !== 'off';
  return (
    <>
      <button
        type="button"
        aria-label={t('sound.open')}
        aria-haspopup="dialog"
        aria-expanded={anchor !== null}
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`flex size-9 items-center justify-center rounded-full ${
          shaped ? 'text-primary' : 'text-muted hover:text-foreground'
        } ${className}`}
      >
        <SlidersVertical aria-hidden size={18} />
      </button>
      {anchor && (
        <Sheet
          title={t('sound.equalizer')}
          anchor={anchor}
          popoverWidth={440}
          onClose={() => setAnchor(null)}
        >
          <div className="flex flex-col gap-4 px-1">
            <EqualizerPanel sound={sound} change={change} />
            <div className="flex flex-wrap gap-2">
              {item && (
                <button
                  type="button"
                  onClick={() => {
                    setAnchor(null);
                    openEditor({
                      title: item.title,
                      artist: item.artist,
                      album: item.album,
                      durationMs: item.durationMs,
                    });
                  }}
                  className="bg-background hover:bg-surface flex items-center gap-2 rounded-full px-3 py-2 text-sm"
                >
                  <Scissors aria-hidden size={14} />
                  {t('sound.editSong')}
                </button>
              )}
              <Link
                to="/settings/sound"
                onClick={() => setAnchor(null)}
                className="bg-background hover:bg-surface flex items-center gap-2 rounded-full px-3 py-2 text-sm"
              >
                <Settings2 aria-hidden size={14} />
                {t('sound.allSettings')}
              </Link>
            </div>
          </div>
        </Sheet>
      )}
    </>
  );
};
