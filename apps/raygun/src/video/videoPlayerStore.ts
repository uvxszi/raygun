import { commands, events } from '@raygun/bindings';
import type { CastStatus, SubtitleMode, VideoChapter } from '@raygun/bindings';
import { useEffect } from 'react';
import { create } from 'zustand';

import { PLAYER_DEFAULTS, withDefaults } from '../settings/playerOptions';
import type { PlayerPrefs } from '../settings/playerOptions';
import type { MediaEngine } from './mediaEngine';
import type {
  EpisodeItem,
  PlayerRequest,
  PlayerUpdate,
  QualityRow,
  SourceGroup,
  TorrentTransfer,
} from './protocol';

const DEFAULT_SEEK_STEP_MS = 10_000;
const DEFAULT_PROMPT_MS = 15_000;

/** A note over the picture: an error from Rust, or one of the player's own. `detail` is the
 * engine's own words, under More info. */
export type Notice = { text: string; detail?: string; sources?: boolean; elsewhere?: boolean };

type VideoPlayerState = {
  open: boolean;
  /** Episodes of a title play through a session (M44): next, previous, episodes, sources. */
  session: boolean;
  title: string;
  subtitle: string | null;
  /** What plays; `null` while a session finds its first link. */
  request: PlayerRequest | null;
  /** Counts loads, so loading the same request again starts it over. */
  loads: number;
  prefs: PlayerPrefs;
  seekStepMs: number;
  /** The app's language; empty when it follows the device. */
  appLanguage: string;
  /** The first subtitle language; empty follows the device's languages, then the app's. */
  subtitleLanguage: string;
  /** When subtitles turn on by themselves. */
  subtitleMode: SubtitleMode;
  autoplay: boolean;
  nextPromptMs: number;
  loadingText: string | null;
  notice: Notice | null;
  episodes: EpisodeItem[];
  hasNext: boolean;
  hasPrevious: boolean;
  sources: SourceGroup[];
  /** The order Sources lists them in by clean name (Rust's `order`). */
  sourceOrder: number[] | null;
  /** The source playing, once it has played a while. */
  playingFrom: string | null;
  /** A link played on its own (Rust's `link`): its own links play first. */
  linkSession: boolean;
  /** It has nothing else to switch to (Rust's `alone`): Sources says "Only this link". */
  sourcesAlone: boolean;
  /** Another link would play if this one failed (Rust's `fallback`). */
  sourcesFallback: boolean;
  /** The Quality menu's steps, as Rust lists them. */
  qualities: QualityRow[];
  /** Why a step tapped in Quality can't play, with when it came (each one shows once). */
  qualityNotice: { text: string; at: number } | null;
  /** The engine playing now (live streams' clock and anchor, the connection's speed). */
  engine: MediaEngine | null;
  chapters: VideoChapter[];
  /** Asks the screen to show Sources (an update said no link worked). */
  showSources: number;
  /** The link loading was picked in Sources, not tried in turn. */
  pickedSource: boolean;
  /** Loop this video (More): on until the player closes, across a session's episodes. */
  looping: boolean;
  /** How the torrent playing is doing, when the video is a torrent's. */
  transfer: TorrentTransfer | null;
  /** What plays on a TV (Play on TV): while there is one, the player is its remote. Kept when
   * the player closes, as the TV plays on. */
  cast: CastStatus | null;
  openWith: (request: PlayerRequest) => void;
  apply: (update: PlayerUpdate) => void;
  setNotice: (notice: Notice | null) => void;
  setLooping: (looping: boolean) => void;
  close: () => void;
};

/** What every request carries (`readSettings` in the Android player). */
const settingsFrom = (request: PlayerRequest, state: VideoPlayerState) => ({
  prefs: request.prefs ? withDefaults(request.prefs) : state.prefs,
  seekStepMs:
    request.seekStepMs !== undefined
      ? Math.min(300_000, Math.max(1_000, request.seekStepMs))
      : state.seekStepMs,
  appLanguage: request.appLanguage ?? state.appLanguage,
  subtitleLanguage: request.subtitleLanguage ?? state.subtitleLanguage,
  subtitleMode: request.subtitleMode ?? state.subtitleMode,
  session: request.session ?? state.session,
  autoplay: request.autoplay ?? state.autoplay,
  nextPromptMs:
    request.nextPromptMs !== undefined
      ? Math.min(60_000, Math.max(0, request.nextPromptMs))
      : state.nextPromptMs,
});

const loaded = (request: PlayerRequest, state: VideoPlayerState) => ({
  ...settingsFrom(request, state),
  request,
  loads: state.loads + 1,
  title: request.title ?? state.title,
  subtitle: request.subtitle ?? null,
  chapters: request.chapters ?? [],
  loadingText: null,
  notice: null,
  transfer: null,
});

const CLOSED = {
  open: false,
  session: false,
  title: '',
  subtitle: null,
  request: null,
  loads: 0,
  prefs: PLAYER_DEFAULTS,
  seekStepMs: DEFAULT_SEEK_STEP_MS,
  appLanguage: '',
  subtitleLanguage: '',
  subtitleMode: 'foreign',
  autoplay: true,
  nextPromptMs: DEFAULT_PROMPT_MS,
  loadingText: null,
  notice: null,
  episodes: [],
  hasNext: false,
  hasPrevious: false,
  sources: [],
  sourceOrder: null,
  playingFrom: null,
  linkSession: false,
  sourcesAlone: false,
  sourcesFallback: true,
  qualities: [],
  qualityNotice: null,
  engine: null,
  chapters: [],
  showSources: 0,
  pickedSource: false,
  looping: false,
  transfer: null,
} satisfies Partial<VideoPlayerState>;

/** The webview's video player (desktop, M46.4), driven by Rust's `videoPlayerEvent`s. */
export const useVideoPlayerStore = create<VideoPlayerState>((set) => ({
  ...CLOSED,
  cast: null,
  openWith: (request) =>
    set((state) => {
      // A new opening starts clean, except for a session's episodes sent just before. The load
      // count goes on, so a video opened over another starts its stage afresh.
      const base = {
        ...state,
        ...CLOSED,
        loads: state.loads,
        open: true,
        session: request.session ?? false,
      };
      if (request.url) return { ...base, ...loaded(request, base) };
      return {
        ...base,
        ...settingsFrom(request, base),
        title: request.title ?? '',
        subtitle: request.subtitle ?? null,
        loadingText: request.loadingText ?? '',
      };
    }),
  apply: (update) =>
    set((state) => {
      if (!state.open) return state;
      switch (update.type) {
        case 'load':
          return loaded(update.request, state);
        case 'loading':
          return { loadingText: update.text, notice: null, pickedSource: false };
        case 'close':
          return { ...CLOSED };
        case 'error':
          return {
            loadingText: null,
            notice: { text: update.text, sources: update.showSources },
            showSources: update.showSources ? state.showSources + 1 : state.showSources,
          };
        case 'episodes':
          return {
            episodes: update.items,
            hasNext: update.hasNext,
            hasPrevious: update.hasPrevious,
            autoplay: update.autoplay ?? state.autoplay,
          };
        case 'sources':
          return {
            sources: update.groups,
            playingFrom: update.playingFrom ?? null,
            sourceOrder: update.order ?? null,
            linkSession: update.link ?? false,
            sourcesAlone: update.alone ?? false,
            sourcesFallback: update.fallback ?? true,
          };
        case 'qualities':
          return {
            qualities: update.items,
            qualityNotice: update.notice ? { text: update.notice, at: Date.now() } : null,
            // A step asked for that nothing else plays: the video playing goes on as it was.
            pickedSource: update.notice ? false : state.pickedSource,
          };
        case 'prefs': {
          // Only what settings say; the session's own fields stay.
          const { prefs, seekStepMs, appLanguage, subtitleLanguage, subtitleMode } = settingsFrom(
            { id: '', ...update },
            state,
          );
          return { prefs, seekStepMs, appLanguage, subtitleLanguage, subtitleMode };
        }
        case 'transfer': {
          if (update.id !== state.request?.id) return state;
          const { peers, seeds, downloadBps, have, len, dataSaver } = update;
          return { transfer: { peers, seeds, downloadBps, have, len, dataSaver } };
        }
        case 'chapters': {
          if (update.id !== state.request?.id) return state;
          // AniSkip's, beside any the source gave.
          const found = update.items.filter((item) =>
            state.chapters.every((chapter) => Math.abs(chapter.startMs - item.startMs) >= 1_000),
          );
          return {
            chapters: [...state.chapters, ...found].sort((a, b) => a.startMs - b.startMs),
          };
        }
        default:
          return state;
      }
    }),
  setNotice: (notice) => set({ notice }),
  setLooping: (looping) => set({ looping }),
  close: () => set(CLOSED),
}));

/** Feeds Rust's messages to the store. Mount once, in the app shell. */
export const useVideoPlayerEvents = () => {
  useEffect(() => {
    // What plays on a TV, also after the webview reloads.
    void commands.castStatus().then((cast) => useVideoPlayerStore.setState({ cast }));
    const unlistenCast = events.castStatusEvent.listen(({ payload }) =>
      useVideoPlayerStore.setState({ cast: payload }),
    );
    const unlisten = events.videoPlayerEvent.listen(({ payload }) => {
      let message: unknown;
      try {
        message = JSON.parse(payload.json);
      } catch {
        return;
      }
      const store = useVideoPlayerStore.getState();
      if (payload.kind === 'open') store.openWith(message as PlayerRequest);
      else store.apply(message as PlayerUpdate);
    });
    return () => {
      void unlisten.then((stop) => stop());
      void unlistenCast.then((stop) => stop());
    };
  }, []);
};
