import type { QueueItem } from '@raygun/bindings';
import { ImageIcon, MonitorPlay } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { coverTitle } from '../music/coverSheetStore';
import { useCoverHold } from '../music/useCoverHold';
import { hasVideo } from './hasVideo';
import { usePlayerUiStore } from './playerUiStore';
import { useVideoSurface } from './useVideoSurface';

type MediaArtProps = {
  item: QueueItem;
  active: boolean;
  artworkClassName: string;
  videoClassName: string;
  iconSize: number;
  testId?: string;
  /** What a tap on the artwork does (the phone's player: its lyrics), named by `tapLabel`. */
  onTap?: () => void;
  tapLabel?: string;
};

/**
 * The artwork, or the video for songs that play through YouTube once asked for (M46.8): music
 * plays as music, and the video shows only when the user presses it. `active` says this is the
 * surface on screen (the full player takes the video from the side panel).
 */
export const MediaArt: FC<MediaArtProps> = (props) => {
  const { item, active, videoClassName, testId } = props;
  const videoShown = usePlayerUiStore((store) => store.videoShown);
  const showing = hasVideo(item) && videoShown;
  const videoRef = useVideoSurface(active && showing);

  if (showing) {
    return (
      <div ref={videoRef} data-testid={testId ?? 'video-surface'} className={videoClassName} />
    );
  }
  return <HeldArtwork {...props} />;
};

/**
 * The artwork. Held (or right-clicked) it shows large, to save (M46.26); a tap does `onTap`, if
 * anything. The video toggle sits beside the title (M46.24: the button over the artwork was in
 * the way).
 */
const HeldArtwork: FC<MediaArtProps> = ({ item, artworkClassName, iconSize, onTap, tapLabel }) => {
  const { t } = useTranslation();
  const cover = item.artworkUrl ? { url: item.artworkUrl, title: coverTitle(item) } : null;
  const hold = useCoverHold({ cover, onTap });
  const artwork = (
    <Artwork
      key={item.artworkUrl ?? 'none'}
      url={item.artworkUrl}
      className={artworkClassName}
      iconSize={iconSize}
    />
  );
  return (
    <div className="relative w-full">
      {cover || onTap ? (
        <button
          type="button"
          aria-label={onTap ? tapLabel : t('cover.open')}
          title={cover ? t('cover.holdHint') : undefined}
          data-testid="player-artwork"
          {...hold.props}
          className="cover-hold block w-full"
        >
          {artwork}
        </button>
      ) : (
        artwork
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
