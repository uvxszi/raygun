import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Maximize2, Pause, Play, Volume2, VolumeX, X } from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useDataSaving, useNetworkStatus } from '../network/useNetworkMode';
import { useSettings } from '../settings/useSettings';
import { useVideoPlayerStore } from './videoPlayerStore';

type Props = {
  /** The YouTube video. */
  videoId: string;
  title: string;
  /** Opens the full trailer player. */
  onFull: () => void;
};

/**
 * A title's trailer in a small faded window on its page (M46.34). It starts muted (webviews
 * don't autoplay sound), can be paused, unmuted, made full or stopped, and pauses when the video
 * player opens. Settings → Player → Trailers on title pages turns it off.
 */
export const TrailerWindow: FC<Props> = ({ videoId, title, onFull }) => {
  const { t } = useTranslation();
  // Never while Offline Mode is on, nor while Data Saver keeps trailers from playing by themselves.
  const offline = useNetworkStatus().offline;
  const saving = useDataSaving('noTrailers');
  const enabled = (useSettings().data?.videoPlayer.titleTrailers ?? false) && !offline && !saving;
  const embed = useQuery({
    queryKey: ['youtubeEmbed', false],
    queryFn: () => commands.youtubeEmbedUrl(false),
    staleTime: Infinity,
    enabled,
  });
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [stopped, setStopped] = useState(false);
  const [failed, setFailed] = useState(false);
  const [playing, setPlaying] = useState(true);
  const [muted, setMuted] = useState(true);
  const playerOpen = useVideoPlayerStore((state) => Boolean(state.request) || state.session);

  const send = (command: Record<string, unknown>) =>
    frameRef.current?.contentWindow?.postMessage({ raygunCommand: true, ...command }, '*');

  const onMessage = useEffectEvent((event: MessageEvent) => {
    const frame = frameRef.current?.contentWindow;
    if (!frame || event.source !== frame) return;
    const data = event.data as { raygunEmbed?: boolean; type?: string; state?: string } | null;
    if (!data?.raygunEmbed) return;
    if (data.type === 'ready') {
      // Back after the film: paused, as it was left.
      send({ type: 'load', videoId, startMs: 0, autoplay: playing, volume: 0 });
    } else if (data.type === 'error') {
      setFailed(true);
    } else if (data.type === 'state' && data.state === 'ended') {
      setStopped(true);
    }
  });
  useEffect(() => {
    const listener = (event: MessageEvent) => onMessage(event);
    window.addEventListener('message', listener);
    return () => window.removeEventListener('message', listener);
  }, []);

  // The film itself starting (or its full trailer) takes over: the button shows paused at once,
  // and the embed is told.
  const [wasOpen, setWasOpen] = useState(playerOpen);
  if (playerOpen !== wasOpen) {
    setWasOpen(playerOpen);
    if (playerOpen) setPlaying(false);
  }
  useEffect(() => {
    if (playerOpen) send({ type: 'pause' });
  }, [playerOpen]);

  // Out of the way while the film or another video plays (M46.40); it comes back after.
  if (!enabled || stopped || failed || !embed.data || playerOpen) return null;

  const toggle = () => {
    send({ type: playing ? 'pause' : 'play' });
    setPlaying(!playing);
  };
  const toggleSound = () => {
    send({ type: 'volume', volume: muted ? 1 : 0 });
    setMuted(!muted);
  };
  const button =
    'grid size-8 place-items-center rounded-full bg-black/55 text-white backdrop-blur hover:bg-black/75';

  return (
    <figure
      data-testid="trailer-window"
      aria-label={t('watch.trailerOf', { title })}
      className="group relative aspect-video w-full overflow-hidden rounded-2xl shadow-2xl shadow-black/40 [mask-image:radial-gradient(ellipse_at_center,black_70%,transparent_100%)]"
    >
      <iframe
        ref={frameRef}
        title={t('watch.trailerOf', { title })}
        src={embed.data}
        allow="autoplay; encrypted-media"
        className="pointer-events-none size-full border-0 bg-black"
      />
      <figcaption className="absolute inset-x-3 bottom-3 flex items-center gap-1.5 opacity-90 transition-opacity group-hover:opacity-100">
        <button
          type="button"
          aria-label={playing ? t('player.pause') : t('player.play')}
          onClick={toggle}
          className={button}
        >
          {playing ? <Pause aria-hidden size={15} /> : <Play aria-hidden size={15} />}
        </button>
        <button
          type="button"
          aria-label={muted ? t('desktopPlayer.unmute') : t('desktopPlayer.mute')}
          onClick={toggleSound}
          className={button}
        >
          {muted ? <VolumeX aria-hidden size={15} /> : <Volume2 aria-hidden size={15} />}
        </button>
        <span className="flex-1" />
        <button
          type="button"
          aria-label={t('watch.trailerFull')}
          onClick={() => {
            send({ type: 'pause' });
            setPlaying(false);
            onFull();
          }}
          className={button}
        >
          <Maximize2 aria-hidden size={15} />
        </button>
        <button
          type="button"
          aria-label={t('watch.trailerStop')}
          onClick={() => {
            send({ type: 'stop' });
            setStopped(true);
          }}
          className={button}
        >
          <X aria-hidden size={15} />
        </button>
      </figcaption>
    </figure>
  );
};
