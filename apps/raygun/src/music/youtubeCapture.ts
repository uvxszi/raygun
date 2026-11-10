import { commands, events } from '@raygun/bindings';
import type { SongCaptureReport, SongCaptureRequest } from '@raygun/bindings';
import { useEffect } from 'react';

import { YOUTUBE_ERRORS } from '../engine/youtubeEngine';
import { toBase64 } from './songCapture';

/**
 * Songs only YouTube's player plays, captured on both platforms (M46.52). Rust asks for a video,
 * and a hidden YouTube player of its own plays it here: Raygun's embed host page in an iframe kept
 * off screen. Raygun's tap script runs inside YouTube's frame (an initialization script of every
 * frame, `music_downloads/capture/youtube_tap.js`): asked through the host page, it hears the
 * player's sound, muted or through Web Audio as Rust asks, and posts pieces of PCM to this page,
 * each saying where in the video it starts. They go on to Rust with `song_capture_report`, as the
 * desktop capture player's do. Nothing is heard, and the player people listen to is never touched.
 */

/** Origins YouTube's player frame has. */
const YOUTUBE_ORIGINS = new Set(['https://www.youtube.com', 'https://www.youtube-nocookie.com']);
/** How long the host page may take to be ready, ms. */
export const READY_MS = 30_000;
/** How long the tap may take to answer once the player is ready, ms. */
export const ARM_MS = 10_000;
/** How long the tap may take to find the player's video before it's loaded anyway, ms. */
export const ATTACH_MS = 4_000;

/** What the tap inside YouTube's frame posts. */
type TapMessage = {
  raygunTap: true;
  session: string;
  type: 'armed' | 'attached' | 'sound' | 'stalled' | 'ended' | 'failed';
  startMs?: number;
  rate?: number;
  channels?: number;
  speed?: number;
  pcm?: ArrayBuffer;
  durationMs?: number | null;
  error?: string | null;
};

/** What the embed host page posts. */
type HostMessage = { raygunEmbed: true; type: string; code?: number };

type Session = {
  id: string;
  request: SongCaptureRequest;
  frame: HTMLIFrameElement;
  hostOrigin: string;
  armed: boolean;
  loaded: boolean;
  stopped: boolean;
  timers: ReturnType<typeof setTimeout>[];
  /** The reports on their way, one after another. */
  sending: Promise<void>;
};

const sessions = new Map<string, Session>();

const isTap = (data: unknown): data is TapMessage =>
  typeof data === 'object' &&
  data !== null &&
  (data as { raygunTap?: unknown }).raygunTap === true &&
  typeof (data as { session?: unknown }).session === 'string';

const isHost = (data: unknown): data is HostMessage =>
  typeof data === 'object' &&
  data !== null &&
  (data as { raygunEmbed?: unknown }).raygunEmbed === true;

/** The hidden player's frame: off screen, still playing (as the video dock's is). */
const makeFrame = (url: string) => {
  const frame = document.createElement('iframe');
  frame.title = 'Capture player';
  frame.allow = 'autoplay; encrypted-media';
  frame.setAttribute('aria-hidden', 'true');
  frame.tabIndex = -1;
  Object.assign(frame.style, {
    position: 'fixed',
    left: '-10000px',
    top: '0',
    width: '320px',
    height: '180px',
    border: '0',
    pointerEvents: 'none',
  });
  frame.src = url;
  document.body.append(frame);
  return frame;
};

const command = (session: Session, message: Record<string, unknown>) => {
  session.frame.contentWindow?.postMessage({ raygunCommand: true, ...message }, session.hostOrigin);
};

/** Sends a report after the ones before it, so they arrive in order. */
const report = (session: Session, sent: Omit<SongCaptureReport, 'session'>) => {
  session.sending = session.sending
    .then(() => commands.songCaptureReport({ ...sent, session: session.id }))
    .then((result) => {
      // Rust stopped this capture.
      if (result.status === 'error') stopTap(session.id);
    })
    .catch(() => stopTap(session.id));
  return session.sending;
};

const fail = (session: Session, error: string) => {
  if (session.stopped) return;
  void report(session, {
    kind: 'failed',
    startMs: 0,
    rate: 48_000,
    channels: 2,
    speed: session.request.speed ?? 1,
    pcm: null,
    durationMs: null,
    error,
  }).then(() => stopTap(session.id));
};

const later = (session: Session, ms: number, run: () => void) => {
  session.timers.push(
    setTimeout(() => {
      if (!session.stopped) run();
    }, ms),
  );
};

/** The video plays from where Rust asked, at the speed the tap hears it at. */
const load = (session: Session, speed: number) => {
  if (session.loaded) return;
  session.loaded = true;
  const { request } = session;
  command(session, {
    type: 'load',
    videoId: request.videoId,
    startMs: request.fromMs ?? 0,
    autoplay: true,
    volume: 1,
    token: 1,
  });
  command(session, { type: 'rate', rate: speed });
  if (request.tapMode === 'muted') command(session, { type: 'mute' });
};

const onHost = (session: Session, data: HostMessage) => {
  if (data.type === 'ready') {
    command(session, {
      type: 'tap',
      tap: 'start',
      session: session.id,
      speed: session.request.speed ?? 1,
      mode: session.request.tapMode ?? 'graph',
    });
    later(session, ARM_MS, () => {
      if (!session.armed) fail(session, 'This device’s webview can’t hear YouTube’s player');
    });
  } else if (data.type === 'error') {
    const code = data.code ?? 0;
    fail(session, YOUTUBE_ERRORS[code] ?? `YouTube player error ${code}`);
  }
};

const onTap = (session: Session, data: TapMessage) => {
  switch (data.type) {
    case 'armed':
      session.armed = true;
      later(session, ATTACH_MS, () => load(session, session.request.speed ?? 1));
      return;
    case 'attached':
      load(session, data.speed ?? session.request.speed ?? 1);
      return;
    default:
      void report(session, {
        kind: data.type,
        startMs: data.startMs ?? 0,
        rate: data.rate ?? 48_000,
        channels: data.channels ?? 2,
        speed: data.speed ?? session.request.speed ?? 1,
        pcm: data.pcm ? toBase64(new Int16Array(data.pcm)) : null,
        durationMs: data.durationMs ?? null,
        error: data.error ?? null,
      });
  }
};

/** Messages from the host pages and from the taps inside their YouTube frames. */
export const onMessage = (event: MessageEvent) => {
  if (isTap(event.data) && YOUTUBE_ORIGINS.has(event.origin)) {
    const session = sessions.get(event.data.session);
    // Only the YouTube frame inside this capture's own player answers for it.
    if (!session || session.stopped || event.source !== session.frame.contentWindow?.[0]) return;
    onTap(session, event.data);
    return;
  }
  if (!isHost(event.data)) return;
  for (const session of sessions.values()) {
    if (event.source === session.frame.contentWindow && event.origin === session.hostOrigin) {
      if (!session.stopped) onHost(session, event.data);
      return;
    }
  }
};

export const startTap = (request: SongCaptureRequest) => {
  if (!request.videoId || !request.embedUrl) return;
  stopTap(request.session);
  const session: Session = {
    id: request.session,
    request,
    frame: makeFrame(request.embedUrl),
    hostOrigin: new URL(request.embedUrl).origin,
    armed: false,
    loaded: false,
    stopped: false,
    timers: [],
    sending: Promise.resolve(),
  };
  sessions.set(session.id, session);
  later(session, READY_MS, () => {
    if (!session.armed) fail(session, 'YouTube’s player didn’t load');
  });
};

export const stopTap = (id: string) => {
  const session = sessions.get(id);
  if (!session || session.stopped) return;
  session.stopped = true;
  sessions.delete(id);
  for (const timer of session.timers) clearTimeout(timer);
  command(session, { type: 'tap', tap: 'stop', session: id });
  command(session, { type: 'stop' });
  session.frame.remove();
};

/** Captures the YouTube videos Rust asks for, on every platform. */
export const useYouTubeCaptureHost = () => {
  useEffect(() => {
    window.addEventListener('message', onMessage);
    const listening = events.songCaptureEvent.listen(({ payload }) => {
      if (payload.videoId) startTap(payload);
      else if (!payload.url) stopTap(payload.session);
    });
    return () => {
      window.removeEventListener('message', onMessage);
      void listening.then((unlisten) => unlisten());
      for (const id of [...sessions.keys()]) stopTap(id);
    };
  }, []);
};
