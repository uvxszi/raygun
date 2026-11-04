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
};

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
  /** `playingFrom`: the source the video plays from, once it has played a while. */
  | { type: 'sources'; groups: SourceGroup[]; playingFrom?: string | null }
  | { type: 'chapters'; id: string; items: VideoChapter[] };

export type ReportState = 'progress' | 'ended' | 'error' | 'switched' | 'closed';

/** Where the video is, for a report. */
export type Position = { positionMs: number; durationMs: number | null };

/** Progress and what happened: saved for resuming; `error` makes a session try the next link. */
export const sendReport = (id: string, state: ReportState, at: Position, error?: string) => {
  if (!id && state !== 'closed') return;
  void commands.videoPlayerReport(
    JSON.stringify({
      id,
      state,
      positionMs: Math.max(0, Math.round(at.positionMs)),
      durationMs: at.durationMs && at.durationMs > 0 ? Math.round(at.durationMs) : null,
      error: error ?? null,
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
