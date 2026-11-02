import type { QueueItem } from '@raygun/bindings';
import { ImageIcon, MonitorPlay } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { hasVideo } from './hasVideo';
import { usePlayerUiStore } from './playerUiStore';
import { useVideoSurface } from './useVideoSurface';

/**
 * The artwork, or the video for songs that play through YouTube once asked for (M46.8): music
 * plays as music, and the video shows only when the user presses it. `active` says this is the
 * surface on screen (the full player takes the video from the side panel).
 */
export const MediaArt: FC<{
  item: QueueItem;
  active: boolean;
  artworkClassName: string;
  videoClassName: string;
  iconSize: number;
  testId?: string;
}> = ({ item, active, artworkClassName, videoClassName, iconSize, testId }) => {
  const { t } = useTranslation();
  const videoShown = usePlayerUiStore((store) => store.videoShown);
  const setVideoShown = usePlayerUiStore((store) => store.setVideoShown);
  const showing = hasVideo(item) && videoShown;
  const videoRef = useVideoSurface(active && showing);

  if (showing) {
    return (
      <div ref={videoRef} data-testid={testId ?? 'video-surface'} className={videoClassName} />
    );
  }
  return (
    <div className="relative w-full">
      <Artwork
        key={item.artworkUrl ?? 'none'}
        url={item.artworkUrl}
        className={artworkClassName}
        iconSize={iconSize}
      />
      {hasVideo(item) && (
        <button
          type="button"
          onClick={() => setVideoShown(true)}
          className="absolute right-3 bottom-3 flex items-center gap-1.5 rounded-full bg-black/70 px-3 py-1.5 text-xs font-semibold text-white backdrop-blur"
        >
          <MonitorPlay aria-hidden size={14} />
          {t('player.showVideo')}
        </button>
      )}
    </div>
  );
};

/** Switches a YouTube song between its artwork and its video. */
export const VideoToggle: FC<{ item: QueueItem | undefined; className?: string }> = ({
  item,
  className = '',
}) => {
  const { t } = useTranslation();
  const videoShown = usePlayerUiStore((store) => store.videoShown);
  const setVideoShown = usePlayerUiStore((store) => store.setVideoShown);
  if (!hasVideo(item)) return null;
  return (
    <button
      type="button"
      aria-label={t(videoShown ? 'player.showArtwork' : 'player.showVideo')}
      aria-pressed={videoShown}
      onClick={() => setVideoShown(!videoShown)}
      className={`grid shrink-0 place-items-center rounded-full ${videoShown ? 'text-primary' : 'text-muted hover:text-foreground'} ${className}`}
    >
      {videoShown ? <ImageIcon aria-hidden size={18} /> : <MonitorPlay aria-hidden size={18} />}
    </button>
  );
};
