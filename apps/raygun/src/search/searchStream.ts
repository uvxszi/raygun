import { events } from '@raygun/bindings';
import type {
  MusicAlbum,
  MusicArtist,
  MusicPlaylist,
  ProviderSearchResult,
  SearchProgress,
} from '@raygun/bindings';
import { hashKey } from '@tanstack/react-query';
import type { QueryKey } from '@tanstack/react-query';
import { useEffect } from 'react';
import { create } from 'zustand';

import { ONLINE_SEARCH } from '../extensions/useExtensions';

/**
 * The query keys of Search's music searches. Rust sends each source's answer as it comes
 * (`SearchProgressEvent`); they collect under the same key until the whole answer arrives.
 */
export const SEARCH_KEYS = {
  songs: (query: string): QueryKey => [...ONLINE_SEARCH, query.trim()],
  /** The same words asked without Search's song filters (Find a source). */
  allSongs: (query: string): QueryKey => [...ONLINE_SEARCH, query.trim(), 'unfiltered'],
  artists: (query: string): QueryKey => ['music', 'searchArtists', query.trim()],
  albums: (query: string): QueryKey => ['music', 'searchAlbums', query.trim()],
  playlists: (query: string, featured: boolean): QueryKey => [
    'music',
    'searchPlaylists',
    query.trim(),
    featured,
  ],
};

/** What a search's sources have answered so far. */
export type StreamedAnswers = {
  /** Sources asked; 0 until the first answers. */
  total: number;
  /** The sources that answered, by provider id. */
  answered: string[];
  songs: ProviderSearchResult[];
  artists: MusicArtist[];
  albums: MusicAlbum[];
  playlists: MusicPlaylist[];
};

const EMPTY: StreamedAnswers = {
  total: 0,
  answered: [],
  songs: [],
  artists: [],
  albums: [],
  playlists: [],
};

const useStream = create<{ answers: Record<string, StreamedAnswers> }>(() => ({ answers: {} }));

/** Searches being asked now, by query hash; answers for anything else came late. */
const live = new Set<string>();

/** How long a finished search's parts stay: the whole answer has taken over by then. */
const KEEP_PARTS_MS = 2_000;

const dropParts = (hash: string) =>
  useStream.setState((state) => {
    if (!(hash in state.answers)) return state;
    const answers = { ...state.answers };
    delete answers[hash];
    return { answers };
  });

/**
 * Runs a streamed search's command (`key` is its query's key): while it runs, its sources'
 * answers collect for `useStreamedAnswers`.
 */
export const streamed = async <T>(key: QueryKey, run: () => Promise<T>): Promise<T> => {
  const hash = hashKey(key);
  live.add(hash);
  dropParts(hash);
  try {
    return await run();
  } finally {
    live.delete(hash);
    setTimeout(() => {
      if (!live.has(hash)) dropParts(hash);
    }, KEEP_PARTS_MS);
  }
};

/** What `key`'s search has collected so far, while it runs. */
export const useStreamedAnswers = (key: QueryKey): StreamedAnswers | undefined => {
  const hash = hashKey(key);
  return useStream((state) => state.answers[hash]);
};

const keyOf = ({ query, part }: SearchProgress): QueryKey => {
  switch (part.kind) {
    case 'songs':
      return part.unfiltered ? SEARCH_KEYS.allSongs(query) : SEARCH_KEYS.songs(query);
    case 'artists':
      return SEARCH_KEYS.artists(query);
    case 'albums':
      return SEARCH_KEYS.albums(query);
    case 'playlists':
      return SEARCH_KEYS.playlists(query, part.featured);
  }
};

/** A source's answer added to what came before (an earlier answer of its replaced). */
const withPart = (answers: StreamedAnswers, progress: SearchProgress): StreamedAnswers => {
  const { part } = progress;
  const provider = part.kind === 'songs' ? part.result.provider.id : part.provider.id;
  const next: StreamedAnswers = {
    ...answers,
    total: progress.total,
    answered: answers.answered.includes(provider)
      ? answers.answered
      : [...answers.answered, provider],
  };
  switch (part.kind) {
    case 'songs':
      next.songs = [
        ...answers.songs.filter((result) => result.provider.id !== provider),
        part.result,
      ];
      break;
    case 'artists':
      next.artists = [
        ...answers.artists.filter((artist) => artist.provider.id !== provider),
        ...part.artists,
      ];
      break;
    case 'albums':
      next.albums = [
        ...answers.albums.filter((album) => album.provider.id !== provider),
        ...part.albums,
      ];
      break;
    case 'playlists':
      next.playlists = [
        ...answers.playlists.filter((playlist) => playlist.provider.id !== provider),
        ...part.playlists,
      ];
      break;
  }
  return next;
};

/** Answers that arrive within this long of each other show together, in one render. */
const BATCH_MS = 60;

/**
 * Follows Rust's search progress while Search is open: each source's answer joins its search's
 * as it comes, a batch at a time so many sources answering together don't redraw the page each.
 */
export const useSearchStream = () => {
  useEffect(() => {
    let pending: SearchProgress[] = [];
    let timer: ReturnType<typeof setTimeout> | undefined;
    const flush = () => {
      timer = undefined;
      const batch = pending;
      pending = [];
      useStream.setState((state) => {
        let answers = state.answers;
        for (const progress of batch) {
          const hash = hashKey(keyOf(progress));
          if (!live.has(hash)) continue;
          answers = { ...answers, [hash]: withPart(answers[hash] ?? EMPTY, progress) };
        }
        return answers === state.answers ? state : { answers };
      });
    };
    const unlisten = events.searchProgressEvent.listen(({ payload }) => {
      pending.push(payload);
      timer ??= setTimeout(flush, BATCH_MS);
    });
    return () => {
      clearTimeout(timer);
      void unlisten.then((stop) => stop());
    };
  }, []);
};
