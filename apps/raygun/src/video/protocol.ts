import { commands } from '@raygun/bindings';
import type {
  SubtitleMode,
  VideoAspect,
  VideoChapter,
  VideoPlayerPrefs,
  VideoSubtitle,
} from '@raygun/bindings';

import type { CueEdits } from './cueEdits';

/**
 * The video player's protocol (M44, M45), as the Android player speaks it and as Rust sends it to
 * the webview's player on desktop (M46.4): a request opens it, updates change what it plays, and
 * reports go back as JSON through `videoPlayerReport`.
 */

/** A remembered track: matched by label, then language. */
export type TrackChoice = { language: string | null; label: string | null };

/** What a title remembers (`video.playerState.<group>` in Rust). Speed is in hundredths. */
export type TitleState = {
  speed?: number;
  aspect?: VideoAspect;
  subtitle?: TrackChoice | { off: true };
  subtitleDelayMs?: number;
  audio?: TrackChoice;
  introMs?: number;
  /** Subtitle lines changed by hand (M46.12), per episode and track. */
  subtitleEdits?: CueEdits;
};

/** A request (`prepare` in `video.rs`), or a session's shell before its first video. */
export type PlayerRequest = {
  id: string;
  url?: string;
  title?: string;
  subtitle?: string | null;
  format?: string | null;
  subtitles?: VideoSubtitle[];
  startMs?: number;
  seekStepMs?: number;
  /** The app's language; empty when it follows the device. */
  appLanguage?: string;
  /** The first subtitle language; empty follows the device's languages, then the app's. */
  subtitleLanguage?: string;
  /** When subtitles turn on by themselves. */
  subtitleMode?: SubtitleMode;
  /** The audio's language when the stream doesn't say (anime: Japanese). */
  spokenLanguage?: string | null;
  group?: string;
  titleState?: TitleState | null;
  chapters?: VideoChapter[];
  anime?: unknown;
  prefs?: VideoPlayerPrefs;
  external?: boolean;
  session?: boolean;
  autoplay?: boolean;
  nextPromptMs?: number;
  loadingText?: string;
  /** A live stream as its source marks it: it starts at its live edge (`VideoRequest::live`). */
  live?: boolean;
};

/** What a link's names say about it (`video_quality::LinkFacts`). */
export type LinkFacts = {
  height: number | null;
  range: QualityRange;
  codec: string | null;
  bitDepth: number | null;
  audio: string | null;
  sizeBytes: number | null;
  release: string | null;
};

export type QualityRange = 'sdr' | 'hdr' | 'dv';

/** A step of the Quality menu (`video_quality::QualityRow`). */
export type QualityRow = {
  /** `<height>:<range>`, as the `quality` request names it. */
  key: string;
  height: number;
  range: QualityRange;
  codec: string | null;
} & (
  | { state: 'playing' | 'variant' | 'missing' }
  | { state: 'thisSource'; group: number; link: number }
  | { state: 'otherSource'; group: number; link: number; source: string }
);

/** One of the playing stream's own variants, for the `variants` request. */
export type VariantInfo = { height: number; range: QualityRange; codec: string | null };

/** Where one live segment starts on the player's clock (`video_audio::LiveAnchor`). */
export type LiveAnchor = { sequence: number; startMs: number };

export type EpisodeItem = {
  label: string;
  watched: boolean;
  downloaded: boolean;
  current: boolean;
};

export type SourceLink = {
  label: string;
  current: boolean;
  failed: boolean;
  /** It played last time for this title. */
  worked?: boolean;
  /** What its names say: resolution, HDR, codec, sound, size, release. */
  facts?: LinkFacts;
  /** The bit rate a file of its size needs over the episode's length, kb/s. */
  bitrateKbps?: number | null;
  /** The connection speed measured while it played, kb/s. */
  speedKbps?: number | null;
};

export type SourceGroup = {
  name: string;
  state: 'resolving' | 'ready' | 'empty' | 'failed';
  error: string | null;
  links: SourceLink[];
  /** It failed lately and hasn't worked since (M46.17): tried last. */
  unreliable?: boolean;
  /** It played last time for this title: tried first. */
  worked?: boolean;
  /** Its best picture height among links that can play. */
  best?: number | null;
  /** How many of its links can play. */
  options?: number;
  /** The connection speed measured while it played, kb/s. */
  speedKbps?: number | null;
};

export type PlayerUpdate =
  | { type: 'load'; request: PlayerRequest }
  | { type: 'loading'; text: string }
  | { type: 'error'; text: string; showSources?: boolean }
  /** The session's link plays elsewhere (a YouTube video in its embed, M46.35). */
  | { type: 'close' }
  | {
      type: 'episodes';
      items: EpisodeItem[];
      hasNext: boolean;
      hasPrevious: boolean;
      autoplay?: boolean;
    }
  /** `playingFrom`: the source the video plays from, once it has played a while; `order`: the
   * order Sources lists them in by clean name. */
  | { type: 'sources'; groups: SourceGroup[]; playingFrom?: string | null; order?: number[] }
  | { type: 'chapters'; id: string; items: VideoChapter[] }
  /** The Quality menu; `notice` when a step tapped can't play. */
  | { type: 'qualities'; items: QualityRow[]; wanted?: string | null; notice?: string | null }
  /** Settings changed in the app while a video plays (`video::settings_changed`): followed at
   * once, as a request's would be. */
  | ({ type: 'prefs' } & Pick<
      PlayerRequest,
      'prefs' | 'seekStepMs' | 'appLanguage' | 'subtitleLanguage' | 'subtitleMode'
    >);

export type ReportState = 'progress' | 'ended' | 'error' | 'switched' | 'closed';

/** Where the video is, for a report. */
export type Position = {
  positionMs: number;
  durationMs: number | null;
  /** The video loops: Rust doesn't count its watched point as finishing it. */
  looping?: boolean;
  /** A live stream: its position is its own clock, never a place to resume. */
  live?: boolean;
  /** Where the segment playing starts on that clock (AI subtitles place the sound by it). */
  liveAnchor?: LiveAnchor | null;
  /** The connection's measured speed, kb/s. */
  bandwidthKbps?: number | null;
};

/** What a report or request says beside the position: live streams and the connection. */
const extras = (at: Position) => ({
  ...(at.live ? { live: true, liveAnchor: at.liveAnchor ?? null } : {}),
  ...(at.bandwidthKbps ? { bandwidthKbps: Math.round(at.bandwidthKbps) } : {}),
});

/** Progress and what happened: saved for resuming; `error` makes a session try the next link. */
export const sendReport = (id: string, state: ReportState, at: Position, error?: string) => {
  if (!id && state !== 'closed') return;
  void commands.videoPlayerReport(
    JSON.stringify({
      id,
      state,
      positionMs: Math.max(0, Math.round(at.positionMs)),
      durationMs: !at.live && at.durationMs && at.durationMs > 0 ? Math.round(at.durationMs) : null,
      error: error ?? null,
      looping: at.looping ?? false,
      ...extras(at),
    }),
  );
};

/** Asks Rust for something: the next or previous episode, a source, a save. */
export const sendRequest = (
  id: string,
  action: string,
  positionMs: number,
  extra: Record<string, unknown> = {},
) => {
  void commands.videoPlayerReport(
    JSON.stringify({
      id,
      state: 'request',
      action,
      positionMs: Math.max(0, Math.round(positionMs)),
      ...extra,
    }),
  );
};
