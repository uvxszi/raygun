/**
 * Puts a stream into the webview's `<video>` (M46.4): plain files directly, HLS through hls.js
 * and DASH through dash.js, each loaded only when needed. Streams come through the proxy's
 * `/media/` route, so playlists and segments carry their headers and CORS.
 */

export type EngineTrack = { id: number; label: string; language: string | null };

export type MediaEngine = {
  kind: 'native' | 'hls' | 'dash';
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

export type EngineEvents = {
  /** Tracks or qualities became known or changed. */
  onTracks: () => void;
  /** The stream can't play; `unsupported` when the format or codec is the reason. */
  onFatal: (message: string, unsupported: boolean) => void;
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

/** `MediaError` codes: the picture or sound can't be decoded, or the format isn't supported. */
const MEDIA_ERR_DECODE = 3;
const MEDIA_ERR_SRC_NOT_SUPPORTED = 4;

const attachNative = (video: HTMLVideoElement, url: string, events: EngineEvents): MediaEngine => {
  const onError = () => {
    const error = video.error;
    if (!error) return;
    const unsupported =
      error.code === MEDIA_ERR_SRC_NOT_SUPPORTED || error.code === MEDIA_ERR_DECODE;
    events.onFatal(error.message || `media error ${error.code}`, unsupported);
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
  hls.on(Hls.Events.MANIFEST_PARSED, events.onTracks);
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
    events.onFatal(data.error?.message || data.details, unsupported);
  });
  hls.subtitleDisplay = false;
  hls.loadSource(url);
  hls.attachMedia(video);
  const label = (name: string | undefined, language: string | undefined, index: number) =>
    name || language || `#${index + 1}`;
  return {
    kind: 'hls',
    audioTracks: () =>
      hls.audioTracks.map((track, index) => ({
        id: index,
        label: label(track.name, track.lang, index),
        language: track.lang ?? null,
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
type DashTrack = { index: number | null; lang: string | null; labels: { text: string }[] };
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
  player.on('streamInitialized', events.onTracks);
  player.on('error', (event) => events.onFatal(event.error?.message ?? 'DASH error', false));
  player.initialize(video, url, false);
  const tracks = (type: 'audio' | 'text') =>
    player.getTracksFor(type).map((track, index) => ({
      id: index,
      label: track.labels[0]?.text || track.lang || `#${index + 1}`,
      language: track.lang,
    }));
  return {
    kind: 'dash',
    ...NONE,
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
