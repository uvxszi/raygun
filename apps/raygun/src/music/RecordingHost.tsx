import { commands, events } from '@raygun/bindings';
import type { EngineReport, SongRecordingRequest } from '@raygun/bindings';
import { CircleStop, Disc3, Mic } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { YouTubeEmbedEngine } from '../engine/youtubeEngine';
import { formatTime } from '../player/formatTime';
import { Sheet } from '../shell/Sheet';
import { SPIN } from './songDownloadState';

/** A song being recorded, as its card shows it. */
type Recording = {
  session: string;
  downloadId: string;
  title: string;
  artist: string | null;
  positionMs: number;
  durationMs: number | null;
};

/** The recording's own YouTube player: off screen, still playing (as the video dock's is). */
const makePlayer = () => {
  const frame = document.createElement('iframe');
  frame.title = 'Recording player';
  frame.allow = 'autoplay; encrypted-media';
  Object.assign(frame.style, {
    position: 'fixed',
    left: '-10000px',
    top: '0',
    width: '320px',
    height: '180px',
    border: '0',
    pointerEvents: 'none',
  });
  document.body.append(frame);
  return frame;
};

/**
 * Songs that only YouTube's player plays, recorded as they play (Android): when some wait, this
 * explains it and asks to allow recording (then Android asks its own questions); Rust then has
 * each played here, in a player of its own and at full volume, while it records Raygun's sound,
 * and a card says which song records, how far it is, with Stop.
 */
export const RecordingHost: FC = () => {
  const { t } = useTranslation();
  const [waiting, setWaiting] = useState<number | null>(null);
  const [recording, setRecording] = useState<Recording | null>(null);

  useEffect(() => {
    let frame: HTMLIFrameElement | null = null;
    let engine: YouTubeEmbedEngine | null = null;
    let session: string | null = null;
    let load = 0;
    const report = ({ event }: EngineReport) => {
      if (!session) return;
      void commands.songRecordingReport({ session, event });
      if (event.type === 'progress') {
        setRecording(
          (shown) =>
            shown && {
              ...shown,
              positionMs: event.positionMs ?? shown.positionMs,
              durationMs: event.durationMs ?? shown.durationMs,
            },
        );
      }
    };
    const play = (request: SongRecordingRequest, videoId: string) => {
      frame ??= makePlayer();
      const player = frame;
      engine ??= new YouTubeEmbedEngine(
        player,
        (handler) => {
          const listener = (message: MessageEvent) => handler(message);
          window.addEventListener('message', listener);
          return () => window.removeEventListener('message', listener);
        },
        report,
      );
      session = request.session;
      load += 1;
      engine.handle({
        type: 'loadYouTube',
        loadId: load,
        embedUrl: request.embedUrl,
        videoId,
        startMs: request.startMs,
        autoplay: true,
        volume: 1,
        gainDb: 0,
        endMs: null,
        fadeMs: 0,
      });
      setRecording({
        session: request.session,
        downloadId: request.downloadId,
        title: request.title,
        artist: request.artist,
        positionMs: request.startMs ?? 0,
        durationMs: null,
      });
    };
    const requests = events.songRecordingEvent.listen(({ payload }) => {
      if (payload.videoId) {
        play(payload, payload.videoId);
      } else if (payload.session === session) {
        session = null;
        engine?.handle({ type: 'stop' });
        setRecording(null);
      }
    });
    const asks = events.recordingConsentEvent.listen(({ payload }) => setWaiting(payload.songs));
    return () => {
      void requests.then((stop) => stop());
      void asks.then((stop) => stop());
      engine?.handle({ type: 'stop' });
      engine?.dispose();
      frame?.remove();
    };
  }, []);

  const answer = (allow: boolean) => {
    setWaiting(null);
    if (allow) void commands.songRecordingConsent();
    else void commands.songRecordingDecline();
  };

  return (
    <>
      {waiting !== null && (
        <Sheet title={t('songs.recording.askTitle')} onClose={() => answer(false)}>
          <div data-testid="recording-consent" className="flex flex-col gap-4 pb-2">
            <div className="flex items-center gap-3">
              <Mic aria-hidden size={28} className="text-primary shrink-0" />
              <p className="font-semibold">
                {t('songs.recording.askQuestion', { count: waiting })}
              </p>
            </div>
            <p className="text-muted text-sm">{t('songs.recording.askExplain')}</p>
            <p className="text-muted text-sm">{t('songs.recording.askAndroid')}</p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => answer(false)}
                className="bg-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('songs.recording.notNow')}
              </button>
              <button
                type="button"
                onClick={() => answer(true)}
                className="bg-primary text-background rounded-full px-4 py-2 text-sm font-semibold"
              >
                {t('songs.recording.allow')}
              </button>
            </div>
          </div>
        </Sheet>
      )}
      {recording && <RecordingCard recording={recording} />}
    </>
  );
};

const RecordingCard: FC<{ recording: Recording }> = ({ recording }) => {
  const { t } = useTranslation();
  const { positionMs, durationMs } = recording;
  const share = durationMs ? Math.min(1, positionMs / durationMs) : 0;
  return (
    <div
      role="status"
      data-testid="recording-card"
      className="bg-surface fixed inset-x-4 bottom-[calc(env(safe-area-inset-bottom)+5.5rem)] z-[70] mx-auto flex max-w-sm items-center gap-3 rounded-2xl p-3 shadow-xl"
    >
      <Disc3 aria-hidden size={20} className={`text-primary shrink-0 ${SPIN}`} />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="truncate text-sm font-semibold">
          {t('songs.recording.now', { title: recording.title })}
        </p>
        <div
          role="progressbar"
          aria-label={t('songs.recording.progress')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(share * 100)}
          className="bg-background h-1 w-full overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full w-full origin-left"
            style={{ transform: `scaleX(${share})` }}
          />
        </div>
        <p className="text-muted text-xs tabular-nums">
          {formatTime(positionMs)}
          {durationMs ? ` / ${formatTime(durationMs)}` : ''} · {t('songs.recording.aloud')}
        </p>
      </div>
      <button
        type="button"
        aria-label={t('songs.recording.stop', { title: recording.title })}
        onClick={() => void commands.songDownloadCancelRecording(recording.downloadId)}
        className="hover:bg-primary/15 shrink-0 rounded-full p-2"
      >
        <CircleStop aria-hidden size={20} />
      </button>
    </div>
  );
};
