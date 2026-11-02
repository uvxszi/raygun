import { events } from '@raygun/bindings';
import type { VideoChapter } from '@raygun/bindings';
import { useEffect } from 'react';
import { create } from 'zustand';

import { PLAYER_DEFAULTS, withDefaults } from '../settings/playerOptions';
import type { PlayerPrefs } from '../settings/playerOptions';
import type { EpisodeItem, PlayerRequest, PlayerUpdate, SourceGroup } from './protocol';

const DEFAULT_SEEK_STEP_MS = 10_000;
const DEFAULT_PROMPT_MS = 15_000;

/** A note over the picture: an error from Rust, or one of the player's own. */
export type Notice = { text: string; sources?: boolean; elsewhere?: boolean };

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
  /** `null`: subtitles start off; else the language to turn on. */
  autoSubtitleLanguage: string | null;
  autoplay: boolean;
  nextPromptMs: number;
  loadingText: string | null;
  notice: Notice | null;
  episodes: EpisodeItem[];
  hasNext: boolean;
  hasPrevious: boolean;
  sources: SourceGroup[];
  chapters: VideoChapter[];
  /** Asks the screen to show Sources (an update said no link worked). */
  showSources: number;
  openWith: (request: PlayerRequest) => void;
  apply: (update: PlayerUpdate) => void;
  setNotice: (notice: Notice | null) => void;
  close: () => void;
};

const deviceLanguage = () => (navigator.language || 'en').split('-')[0] ?? 'en';

/** What every request carries (`readSettings` in the Android player). */
const settingsFrom = (request: PlayerRequest, state: VideoPlayerState) => ({
  prefs: request.prefs ? withDefaults(request.prefs) : state.prefs,
  seekStepMs:
    request.seekStepMs !== undefined
      ? Math.min(300_000, Math.max(1_000, request.seekStepMs))
      : state.seekStepMs,
  autoSubtitleLanguage:
    'autoSubtitleLanguage' in request
      ? request.autoSubtitleLanguage === null || request.autoSubtitleLanguage === undefined
        ? null
        : request.autoSubtitleLanguage || deviceLanguage()
      : state.autoSubtitleLanguage,
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
  autoSubtitleLanguage: null,
  autoplay: true,
  nextPromptMs: DEFAULT_PROMPT_MS,
  loadingText: null,
  notice: null,
  episodes: [],
  hasNext: false,
  hasPrevious: false,
  sources: [],
  chapters: [],
  showSources: 0,
} satisfies Partial<VideoPlayerState>;

/** The webview's video player (desktop, M46.4), driven by Rust's `videoPlayerEvent`s. */
export const useVideoPlayerStore = create<VideoPlayerState>((set) => ({
  ...CLOSED,
  openWith: (request) =>
    set((state) => {
      // A new opening starts clean, except for a session's episodes sent just before.
      const base = { ...state, ...CLOSED, open: true, session: request.session ?? false };
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
          return { loadingText: update.text, notice: null };
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
          return { sources: update.groups };
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
  close: () => set(CLOSED),
}));

/** Feeds Rust's messages to the store. Mount once, in the app shell. */
export const useVideoPlayerEvents = () => {
  useEffect(() => {
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
    };
  }, []);
};
