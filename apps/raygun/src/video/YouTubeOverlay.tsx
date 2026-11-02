import type { YouTubeVideo } from '@raygun/bindings';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ArrowLeft, Maximize, Minimize } from 'lucide-react';
import { useEffect, useEffectEvent, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrailerStore } from './trailerStore';
import { SourceIssue } from '../shell/SourceIssue';

/**
 * Trailers and YouTube streams in a full-window player with YouTube's own controls, instead of
 * the music queue's small video (M46.4). The embed host page (M08) is asked for its controls.
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

  const close = () => {
    if (fullscreen) void setWindowFullscreen(false);
    useTrailerStore.getState().close();
  };
  const toggleFullscreen = () => {
    void setWindowFullscreen(!fullscreen);
    setFullscreen(!fullscreen);
  };

  const onMessage = useEffectEvent((event: MessageEvent) => {
    const frame = frameRef.current?.contentWindow;
    if (!frame || event.source !== frame) return;
    const data = event.data as { raygunEmbed?: boolean; type?: string } | null;
    if (!data?.raygunEmbed) return;
    if (data.type === 'ready') {
      frame.postMessage(
        {
          raygunCommand: true,
          type: 'load',
          videoId: video.videoId,
          startMs: 0,
          autoplay: true,
          volume: 1,
        },
        '*',
      );
    } else if (data.type === 'error') {
      setFailed(true);
    }
  });
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
      className="fixed inset-0 z-[75] flex flex-col bg-black text-white"
    >
      <div className="flex items-center gap-2 px-3 py-2 pt-[max(0.5rem,env(safe-area-inset-top))]">
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
