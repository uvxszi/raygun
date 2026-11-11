import type { YouTubeVideo } from '@raygun/bindings';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ArrowLeft, Maximize, Minimize } from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrailerStore } from './trailerStore';
import { useBackLayer } from '../shell/backLayers';
import { SourceIssue } from '../shell/SourceIssue';
import { showToast } from '../shell/toastStore';
import { PartSkipper, partsCommand, spanOf } from '../youtube/skipParts';
import { useSponsorParts } from '../youtube/useSponsorParts';

/**
 * Trailers and YouTube streams in a full-window player with YouTube's own controls, instead of
 * the music queue's small video (M46.4). The embed host page (M08) is asked for its controls.
 * SponsorBlock's parts are skipped as the player reaches them, with a note that takes the skip
 * back, and marked on YouTube's own seek bar.
 */
export const YouTubeOverlay: FC = () => {
  const video = useTrailerStore((state) => state.video);
  return video ? <TrailerScreen key={video.videoId} video={video} /> : null;
};

const setWindowFullscreen = async (on: boolean) => {
  try {
    await getCurrentWindow().setFullscreen(on);
  } catch {
    // Not in Tauri (tests): the player still fills the window.
  }
};

const TrailerScreen: FC<{ video: YouTubeVideo }> = ({ video }) => {
  const { t } = useTranslation();
  const frameRef = useRef<HTMLIFrameElement>(null);
  const [failed, setFailed] = useState(false);
  const [fullscreen, setFullscreen] = useState(false);
  const [durationMs, setDurationMs] = useState<number | null>(null);
  const parts = useSponsorParts(video.videoId, durationMs);
  const [skipper] = useState(() => new PartSkipper());

  const send = (command: Record<string, unknown>) =>
    frameRef.current?.contentWindow?.postMessage({ raygunCommand: true, ...command }, '*');

  const close = () => {
    if (fullscreen) void setWindowFullscreen(false);
    useTrailerStore.getState().close();
  };
  // Back closes it; it takes Escape itself (full screen first).
  useBackLayer(true, close, { escape: false });
  const toggleFullscreen = () => {
    void setWindowFullscreen(!fullscreen);
    setFullscreen(!fullscreen);
  };

  const onMessage = useEffectEvent((event: MessageEvent) => {
    const frame = frameRef.current?.contentWindow;
    if (!frame || event.source !== frame) return;
    const data = event.data as {
      raygunEmbed?: boolean;
      type?: string;
      positionMs?: number;
      durationMs?: number | null;
    } | null;
    if (!data?.raygunEmbed) return;
    if (data.type === 'ready') {
      send({ type: 'load', videoId: video.videoId, startMs: 0, autoplay: true, volume: 1 });
      send(partsCommand(video.videoId, parts));
    } else if (data.type === 'progress' && typeof data.positionMs === 'number') {
      const length = data.durationMs ?? null;
      if (length && length > 0 && durationMs === null) setDurationMs(length);
      const skip = skipper.due(parts, data.positionMs, length, performance.now());
      if (!skip) return;
      send({ type: 'seek', positionMs: skip.toMs });
      showToast(
        t('sponsorBlock.skipped', { part: t(`sponsorBlock.categories.${skip.segment.category}`) }),
        () => {
          skipper.undo(skip.segment);
          send({ type: 'seek', positionMs: spanOf(skip.segment).startMs });
        },
      );
    } else if (data.type === 'error') {
      setFailed(true);
    }
  });
  // YouTube's seek bar marks the parts once they're known (Raygun's script in its frame).
  const sendParts = useEffectEvent(() => send(partsCommand(video.videoId, parts)));
  useEffect(() => {
    if (parts.length > 0) sendParts();
  }, [parts]);
  const onKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.key !== 'Escape') return;
    if (fullscreen) toggleFullscreen();
    else close();
  });
  useEffect(() => {
    const message = (event: MessageEvent) => onMessage(event);
    const key = (event: KeyboardEvent) => onKey(event);
    window.addEventListener('message', message);
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('message', message);
      window.removeEventListener('keydown', key);
    };
  }, []);

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={video.subtitle ? `${video.subtitle} · ${video.title}` : video.title}
      // Clear of a cutout and a navigation bar at the sides, and of the gesture area below.
      className="fixed inset-0 z-[75] flex flex-col bg-black pr-[var(--safe-right)] pb-[var(--safe-bottom)] pl-[var(--safe-left)] text-white"
    >
      <div className="flex items-center gap-2 px-3 py-2 pt-[max(0.5rem,var(--safe-top))]">
        <button
          type="button"
          aria-label={t('desktopPlayer.close')}
          onClick={close}
          className="grid size-10 place-items-center rounded-full hover:bg-white/10"
        >
          <ArrowLeft aria-hidden size={22} />
        </button>
        <div className="flex min-w-0 flex-1 flex-col">
          <h1 className="truncate text-base font-semibold">{video.title}</h1>
          {video.subtitle && <p className="truncate text-sm text-white/70">{video.subtitle}</p>}
        </div>
        <button
          type="button"
          aria-label={
            fullscreen ? t('desktopPlayer.exitFullscreen') : t('desktopPlayer.fullscreen')
          }
          onClick={toggleFullscreen}
          className="grid size-10 place-items-center rounded-full hover:bg-white/10"
        >
          {fullscreen ? <Minimize aria-hidden size={20} /> : <Maximize aria-hidden size={20} />}
        </button>
      </div>
      <div className="grid min-h-0 flex-1 place-items-center px-2 pb-2">
        {failed ? (
          <SourceIssue>{t('desktopPlayer.trailerFailed')}</SourceIssue>
        ) : (
          <iframe
            ref={frameRef}
            title={t('desktopPlayer.trailerPlayer')}
            src={video.embedUrl}
            allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
            allowFullScreen
            className="aspect-video max-h-full w-full max-w-[calc((100vh-5rem)*16/9)] rounded-xl border-0 bg-black"
          />
        )}
      </div>
    </div>
  );
};
