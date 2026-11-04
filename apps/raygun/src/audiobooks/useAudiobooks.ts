import { commands, events } from '@raygun/bindings';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { rememberPlay } from '../music/playContext';

const BOOKS = ['audiobooks'] as const;

/** Books and chapters change with playback and downloads; podcast events cover both (M46.22). */
export const useAudiobookEvents = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.podcastsChangedEvent.listen(() => {
      void client.invalidateQueries({ queryKey: BOOKS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
};

/** The books in the library, the last listened first. */
export const useAudiobooks = () => {
  useAudiobookEvents();
  return useQuery({
    queryKey: [...BOOKS, 'library'],
    queryFn: () => unwrapResult(commands.audiobooks()),
  });
};

/** One book with its chapters, as its order lists them. */
export const useAudiobook = (id: string) => {
  useAudiobookEvents();
  return useQuery({
    queryKey: [...BOOKS, 'book', id],
    queryFn: () => unwrapResult(commands.audiobook(id)),
    enabled: id !== '',
  });
};

/** Opens a book from a directory by its chapter feed; returns its id. */
export const useOpenAudiobook = () =>
  useMutation({
    mutationFn: (feedUrl: string) => unwrapResult(commands.audiobookOpen(feedUrl)),
  });

/** Plays a book from a chapter (or where it was left), with the rest of the book after it. */
export const usePlayAudiobook = () =>
  useMutation({
    mutationFn: async ({ id, chapterId }: { id: string; chapterId: string | null }) => {
      await rememberPlay();
      return unwrapResult(commands.audiobookPlay(id, chapterId));
    },
  });

/** Adds a book to the library or takes it out. */
export const useKeepAudiobook = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ id, keep }: { id: string; keep: boolean }) =>
      unwrapResult(commands.podcastSetSubscribed(id, keep)),
    onSettled: () => void client.invalidateQueries({ queryKey: BOOKS }),
  });
};

/** Downloads every chapter not downloaded yet, two at a time (M46.48). */
export const useDownloadAudiobook = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.audiobookDownload(id)),
    onSettled: () => void client.invalidateQueries({ queryKey: BOOKS }),
  });
};

export type BookList = 'popular' | 'latest' | 'search' | 'subject';

/**
 * Asks a directory list again when a source answered after it showed (M46.48): its late books
 * are kept ready in Rust by then.
 */
const useLateBooks = (list: BookList, query: string) => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.audiobooksFoundEvent.listen(({ payload }) => {
      const [foundList, foundQuery] = payload;
      if (foundList === list && foundQuery === query.trim()) {
        void client.invalidateQueries({ queryKey: [...BOOKS, 'directory', list, query] });
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client, list, query]);
};

/**
 * Every audiobook source's books at once, a page at a time: popular, newest, a search (ranked by
 * title and author) or a genre (`query`: "Mystery", "Science fiction"…). `enabled` holds the
 * request (Home's row asks once it comes near); what's cached shows either way.
 */
export const useBookDirectory = (list: BookList, query = '', enabled = true) => {
  useLateBooks(list, query);
  return useInfiniteQuery({
    queryKey: [...BOOKS, 'directory', list, query],
    queryFn: ({ pageParam }) =>
      unwrapResult(commands.audiobookDirectory(list, query.trim(), pageParam)),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasMore ? pages.length + 1 : undefined),
    enabled: enabled && ((list !== 'search' && list !== 'subject') || query.trim() !== ''),
    staleTime: 10 * 60_000,
  });
};
