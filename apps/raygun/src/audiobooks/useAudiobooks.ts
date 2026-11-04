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

/** One book with its chapters in order. */
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
    mutationFn: ({ id, chapterId }: { id: string; chapterId: string | null }) => {
      rememberPlay();
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

export type BookList = 'popular' | 'latest' | 'search';

/**
 * A directory's books, a page at a time: popular, newest, or a search. `enabled` holds the
 * request (Home's row asks once it comes near); what's cached shows either way.
 */
export const useBookDirectory = (list: BookList, query = '', enabled = true) =>
  useInfiniteQuery({
    queryKey: [...BOOKS, 'directory', list, query],
    queryFn: ({ pageParam }) => unwrapResult(commands.audiobookDirectory(list, query, pageParam)),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasMore ? pages.length + 1 : undefined),
    enabled: enabled && (list !== 'search' || query.trim() !== ''),
    staleTime: 10 * 60_000,
  });
