import { commands } from '@raygun/bindings';
import { Clapperboard, ListPlus, Music, Play } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { usePlayLink } from '../watch/useWatch';
import { toQueueItem } from './toQueueItem';

type Kind = 'video' | 'audio';

const FIELD = 'bg-background placeholder:text-muted rounded-xl px-4 py-3 text-sm outline-none';

/**
 * Plays any address: a video in the player (with an optional subtitle file) or audio in the music
 * queue (with an optional Referer). Opened from the Library's menu.
 */
export const OpenLinkSheet: FC<{ onClose: () => void; initialKind?: Kind }> = ({
  onClose,
  initialKind = 'video',
}) => {
  const { t } = useTranslation();
  const [kind, setKind] = useState<Kind>(initialKind);
  const [location, setLocation] = useState('');
  const [extra, setExtra] = useState('');
  const playLink = usePlayLink();

  const playAudio = (playNow: boolean) => {
    const item = toQueueItem(location, extra);
    if (!item) return;
    void commands.playbackEnqueue([item], playNow);
    onClose();
  };

  const onSubmit = (event: FormEvent) => {
    event.preventDefault();
    if (!location.trim()) return;
    if (kind === 'audio') {
      playAudio(true);
    } else {
      playLink.mutate(
        { url: location.trim(), subtitleUrl: extra.trim() || null },
        { onSuccess: onClose },
      );
    }
  };

  return (
    <Sheet title={t('library.openTitle')} onClose={onClose}>
      <div
        role="radiogroup"
        aria-label={t('library.openKind')}
        className="bg-background flex rounded-full p-1"
      >
        {(['video', 'audio'] as const).map((option) => (
          <button
            key={option}
            type="button"
            role="radio"
            aria-checked={kind === option}
            onClick={() => {
              setKind(option);
              setExtra('');
            }}
            className={`flex flex-1 items-center justify-center gap-2 rounded-full py-2 text-sm font-medium ${kind === option ? 'bg-primary/20 text-primary' : 'text-muted'}`}
          >
            {option === 'video' ? (
              <Clapperboard aria-hidden size={16} />
            ) : (
              <Music aria-hidden size={16} />
            )}
            {t(`library.openAs.${option}`)}
          </button>
        ))}
      </div>
      <form onSubmit={onSubmit} data-testid="open-media-form" className="flex flex-col gap-3">
        <input
          autoFocus
          value={location}
          onChange={(event) => setLocation(event.target.value)}
          placeholder={kind === 'video' ? 'https://…/video.m3u8' : t('library.openPlaceholder')}
          aria-label={kind === 'video' ? t('watch.videoUrl') : t('library.openLocation')}
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          className={FIELD}
        />
        <input
          value={extra}
          onChange={(event) => setExtra(event.target.value)}
          placeholder={
            kind === 'video' ? t('watch.subtitlePlaceholder') : t('library.openRefererPlaceholder')
          }
          aria-label={kind === 'video' ? t('watch.subtitleUrl') : t('library.openReferer')}
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          className={FIELD}
        />
        <p className="text-muted text-xs">
          {kind === 'video' ? t('watch.playLinkHint') : t('library.openAudioHint')}
        </p>
        {playLink.error && (
          <p role="alert" className="text-primary text-sm wrap-anywhere">
            {playLink.error.message}
          </p>
        )}
        <div className="flex justify-end gap-2">
          {kind === 'audio' && (
            <button
              type="button"
              disabled={!location.trim()}
              onClick={() => playAudio(false)}
              className="bg-background flex items-center gap-2 rounded-full px-4 py-2.5 text-sm disabled:opacity-40"
            >
              <ListPlus aria-hidden size={16} />
              {t('library.openQueue')}
            </button>
          )}
          <button
            type="submit"
            disabled={!location.trim() || playLink.isPending}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold disabled:opacity-40"
          >
            <Play aria-hidden size={16} />
            {t('watch.play')}
          </button>
        </div>
      </form>
    </Sheet>
  );
};
