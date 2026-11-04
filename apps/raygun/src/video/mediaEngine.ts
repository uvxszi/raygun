/**
 * Puts a stream into the webview's `<video>` (M46.4): plain files directly, HLS through hls.js
 * and DASH through dash.js, each loaded only when needed. Streams come through the proxy's
 * `/media/` route, so playlists and segments carry their headers and CORS.
 */

export type EngineTrack = {
  id: number;
  label: string;
  language: string | null;
  /** Marked default (HLS `DEFAULT`) or main (DASH's role). */
  original?: boolean;
  /** Only what's said in other languages, and signs. */
  forced?: boolean;
  /** Commentary or audio description. */
  extra?: boolean;
};

export type MediaEngine = {
  kind: 'native' | 'hls' | 'dash';
  /** The stream's own tracks are known (its playlist or manifest was read). */
  tracksKnown: () => boolean;
  audioTracks: () => EngineTrack[];
  audioTrack: () => number;
  setAudioTrack: (id: number) => void;
  /** Subtitle tracks inside the stream; their cues show through hidden text tracks. */
  textTracks: () => EngineTrack[];
  setTextTrack: (id: number) => void;
  /** Heights or bit rates to pick from; `-1` is automatic. */
  qualities: () => EngineTrack[];
  quality: () => number;
  setQuality: (id: number) => void;
  destroy: () => void;
};

/** Why a stream can't play, in the viewer's terms: its format or codec, the network, or its
 * server (refusing it, not having it, failing). */
export type StreamProblem = 'unsupported' | 'network' | 'refused' | 'gone' | 'server' | 'other';

export type EngineEvents = {
  /** Tracks or qualities became known or changed. */
  onTracks: () => void;
  /** The stream can't play; `message` is the engine's own words. */
  onFatal: (message: string, problem: StreamProblem) => void;
};

/** An HTTP status as a problem. */
const httpProblem = (status: number | undefined): StreamProblem => {
  if (status === 401 || status === 403) return 'refused';
  if (status === 404 || status === 410) return 'gone';
  if (status !== undefined && status >= 500) return 'server';
  return 'network';
};

const extension = (url: string) => {
  const path = url.split(/[?#]/)[0] ?? '';
  return path.slice(path.lastIndexOf('.') + 1).toLowerCase();
};

/** How a stream plays: the request's format when it gives one, else the address's extension. */
export const engineKind = (url: string, format?: string | null): MediaEngine['kind'] => {
  const given = format?.toLowerCase();
  if (given === 'hls' || given === 'm3u8') return 'hls';
  if (given === 'dash' || given === 'mpd') return 'dash';
  const ext = extension(url);
  if (ext === 'm3u8' || ext === 'm3u') return 'hls';
  if (ext === 'mpd') return 'dash';
  return 'native';
};

const NONE = {
  tracksKnown: () => false,
  audioTracks: () => [],
  audioTrack: () => -1,
  setAudioTrack: () => {},
  textTracks: () => [],
  setTextTrack: () => {},
  qualities: () => [],
  quality: () => -1,
  setQuality: () => {},
};

/** Hidden, so the player draws their cues itself. */
const hideTextTracks = (video: HTMLVideoElement) => {
  for (const track of Array.from(video.textTracks ?? [])) {
    if (track.mode === 'showing') track.mode = 'hidden';
  }
};

/** `MediaError` codes: the network failed, the picture or sound can't be decoded, or the format
 * isn't supported. */
const MEDIA_ERR_NETWORK = 2;
const MEDIA_ERR_DECODE = 3;
const MEDIA_ERR_SRC_NOT_SUPPORTED = 4;

const attachNative = (video: HTMLVideoElement, url: string, events: EngineEvents): MediaEngine => {
  const onError = () => {
    const error = video.error;
    if (!error) return;
    const problem: StreamProblem =
      error.code === MEDIA_ERR_SRC_NOT_SUPPORTED || error.code === MEDIA_ERR_DECODE
        ? 'unsupported'
        : error.code === MEDIA_ERR_NETWORK
          ? 'network'
          : 'other';
    events.onFatal(error.message || `media error ${error.code}`, problem);
  };
  video.addEventListener('error', onError);
  video.src = url;
  return {
    kind: 'native',
    ...NONE,
    destroy: () => {
      video.removeEventListener('error', onError);
      video.removeAttribute('src');
      video.load();
    },
  };
};

const attachHls = async (
  video: HTMLVideoElement,
  url: string,
  events: EngineEvents,
): Promise<MediaEngine> => {
  const { default: Hls } = await import('hls.js');
  if (!Hls.isSupported()) {
    // A webview with HLS of its own.
    return attachNative(video, url, events);
  }
  const hls = new Hls({ enableWorker: true, renderTextTracksNatively: true });
  let recovered = false;
  let parsed = false;
  hls.on(Hls.Events.MANIFEST_PARSED, () => {
    parsed = true;
    events.onTracks();
  });
  hls.on(Hls.Events.AUDIO_TRACKS_UPDATED, events.onTracks);
  hls.on(Hls.Events.SUBTITLE_TRACKS_UPDATED, events.onTracks);
  hls.on(Hls.Events.LEVEL_SWITCHED, events.onTracks);
  hls.on(Hls.Events.SUBTITLE_TRACK_SWITCH, () => hideTextTracks(video));
  hls.on(Hls.Events.ERROR, (_event, data) => {
    if (!data.fatal) return;
    // One try to get going again, as hls.js advises.
    if (!recovered && data.type === Hls.ErrorTypes.MEDIA_ERROR) {
      recovered = true;
      hls.recoverMediaError();
      return;
    }
    if (
      !recovered &&
      data.type === Hls.ErrorTypes.NETWORK_ERROR &&
      data.details !== 'manifestLoadError'
    ) {
      recovered = true;
      hls.startLoad();
      return;
    }
    const unsupported =
      data.details === Hls.ErrorDetails.MANIFEST_INCOMPATIBLE_CODECS_ERROR ||
      data.details === Hls.ErrorDetails.BUFFER_INCOMPATIBLE_CODECS_ERROR ||
      data.details === Hls.ErrorDetails.MANIFEST_PARSING_ERROR;
    const status = data.response?.code;
    const problem: StreamProblem = unsupported
      ? 'unsupported'
      : data.type === Hls.ErrorTypes.NETWORK_ERROR
        ? httpProblem(status || undefined)
        : 'other';
    const words = data.error?.message || data.details;
    events.onFatal(status ? `${words} (HTTP ${status})` : words, problem);
  });
  hls.subtitleDisplay = false;
  hls.loadSource(url);
  hls.attachMedia(video);
  const label = (name: string | undefined, language: string | undefined, index: number) =>
    name || language || `#${index + 1}`;
  const describes = (characteristics: string | undefined) =>
    Boolean(characteristics?.includes('describes-video'));
  return {
    kind: 'hls',
    tracksKnown: () => parsed,
    audioTracks: () =>
      hls.audioTracks.map((track, index) => ({
        id: index,
        label: label(track.name, track.lang, index),
        language: track.lang ?? null,
        original: track.default,
        extra: describes(track.characteristics),
      })),
    audioTrack: () => hls.audioTrack,
    setAudioTrack: (id) => {
      hls.audioTrack = id;
    },
    textTracks: () =>
      hls.subtitleTracks.map((track, index) => ({
        id: index,
        label: label(track.name, track.lang, index),
        language: track.lang ?? null,
        original: track.default,
        forced: track.forced,
      })),
    setTextTrack: (id) => {
      hls.subtitleTrack = id;
      hls.subtitleDisplay = false;
      hideTextTracks(video);
    },
    qualities: () =>
      hls.levels.map((level, index) => ({
        id: index,
        label: level.height ? `${level.height}p` : `${Math.round(level.bitrate / 1000)} kb/s`,
        language: null,
      })),
    quality: () => (hls.autoLevelEnabled ? -1 : hls.currentLevel),
    setQuality: (id) => {
      hls.currentLevel = id;
    },
    destroy: () => {
      hls.destroy();
      video.removeAttribute('src');
      video.load();
    },
  };
};

/** The parts of dash.js the player uses. */
type DashDescriptor = { value?: string | null } | string;
type DashTrack = {
  index: number | null;
  lang: string | null;
  labels: { text: string }[];
  roles?: DashDescriptor[] | null;
  accessibility?: DashDescriptor[] | null;
};
type DashPlayer = {
  initialize: (view: HTMLVideoElement, url: string, autoPlay: boolean) => void;
  on: (type: string, listener: (event: { error?: { message?: string } }) => void) => void;
  getTracksFor: (type: 'audio' | 'text') => DashTrack[];
  getCurrentTrackFor: (type: 'audio' | 'text') => DashTrack | null;
  setCurrentTrack: (track: DashTrack) => void;
  setTextTrack: (index: number) => void;
  enableText: (enable: boolean) => boolean;
  reset: () => void;
};

const attachDash = async (
  video: HTMLVideoElement,
  url: string,
  events: EngineEvents,
): Promise<MediaEngine> => {
  const dash = (await import('dashjs')) as unknown as {
    MediaPlayer: () => { create: () => DashPlayer };
  };
  const player = dash.MediaPlayer().create();
  let initialized = false;
  player.on('streamInitialized', () => {
    initialized = true;
    events.onTracks();
  });
  player.on('error', (event) => events.onFatal(event.error?.message ?? 'DASH error', 'other'));
  player.initialize(video, url, false);
  // Roles and accessibility are `{ value }` descriptors in dash.js 5, plain strings before.
  const values = (list: DashDescriptor[] | null | undefined) =>
    (list ?? []).map((item) =>
      (typeof item === 'string' ? item : (item.value ?? '')).toLowerCase(),
    );
  const tracks = (type: 'audio' | 'text') =>
    player.getTracksFor(type).map((track, index) => {
      const roles = values(track.roles);
      return {
        id: index,
        label: track.labels[0]?.text || track.lang || `#${index + 1}`,
        language: track.lang,
        original: roles.includes('main'),
        forced: roles.includes('forced-subtitle'),
        extra:
          roles.includes('commentary') ||
          roles.includes('description') ||
          values(track.accessibility).some((value) => value === 'description' || value === '1'),
      };
    });
  return {
    kind: 'dash',
    ...NONE,
    tracksKnown: () => initialized,
    audioTracks: () => tracks('audio'),
    audioTrack: () => {
      const current = player.getCurrentTrackFor('audio');
      return current ? player.getTracksFor('audio').indexOf(current) : -1;
    },
    setAudioTrack: (id) => {
      const track = player.getTracksFor('audio')[id];
      if (track) player.setCurrentTrack(track);
    },
    textTracks: () => tracks('text'),
    setTextTrack: (id) => {
      player.enableText(id >= 0);
      player.setTextTrack(id);
      hideTextTracks(video);
    },
    destroy: () => {
      player.reset();
    },
  };
};

/** Starts `url` in `video`. */
export const attachMedia = (
  video: HTMLVideoElement,
  url: string,
  kind: MediaEngine['kind'],
  events: EngineEvents,
): Promise<MediaEngine> => {
  switch (kind) {
    case 'hls':
      return attachHls(video, url, events);
    case 'dash':
      return attachDash(video, url, events);
    default:
      return Promise.resolve(attachNative(video, url, events));
  }
};

/** The lines of the stream's own subtitles showing now (hidden text tracks). */
export const embeddedCueLines = (video: HTMLVideoElement): string[] => {
  const lines: string[] = [];
  for (const track of Array.from(video.textTracks ?? [])) {
    if (track.mode !== 'hidden' || !track.activeCues) continue;
    for (const cue of Array.from(track.activeCues)) {
      const text = (cue as VTTCue).text ?? '';
      lines.push(
        ...text
          .replace(/<[^>]*>/g, '')
          .split('\n')
          .map((line) => line.trim())
          .filter(Boolean),
      );
    }
  }
  return lines;
};
