import type { MangaCard, OnlineTrack } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';

import type { Medium } from '../manga/medium';
import { collectFromCache } from '../shell/queryMemory';
import { keepTrack, useSearchFilter } from './searchFilter';
import { videoTitleScore } from '../watch/videoMatch';
import { titleScore } from './rankResults';

/** A cached thing counts as an answer when its name shares every word of the query. */
const GOOD_ENOUGH = 60;
/** At most this many shown before the sources answer. */
const SHOWN = 20;

/** Answers holding songs: charts, genres, albums, playlists, mixes, artists, earlier searches. */
const SONG_ROOTS = [['music'], ['onlineSearch']];

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const asTrack = (record: Record<string, unknown>): OnlineTrack | null =>
  typeof record.title === 'string' &&
  'externalId' in record &&
  'source' in record &&
  'artworkUrl' in record
    ? (record as unknown as OnlineTrack)
    : null;

const songScore = (track: OnlineTrack, query: string) =>
  Math.max(
    titleScore(track.title, query),
    titleScore(`${track.artist ?? ''} ${track.title}`, query),
    titleScore(`${track.title} ${track.artist ?? ''}`, query),
  );

const NO_SONGS: OnlineTrack[] = [];
const NO_CARDS: MangaCard[] = [];

/**
 * Songs this device already has answers about (M46.47: Home's charts, genres, albums, earlier
 * searches) that answer `query`, best first: shown at once while the sources are asked. Looked
 * for only while `needed` (the sources haven't answered): the cache is read whole.
 */
export const useSeenSongs = (query: string, needed = true): OnlineTrack[] => {
  const client = useQueryClient();
  const filter = useSearchFilter();
  const wanted = query.trim();
  return useMemo(() => {
    if (!needed || wanted.length < 2) return NO_SONGS;
    const key = (track: OnlineTrack) => `${simple(track.artist)}|${simple(track.title)}`;
    const tracks = new Map<string, OnlineTrack>();
    for (const root of SONG_ROOTS) {
      for (const track of collectFromCache(client, root, asTrack, key)) {
        // Search's filters: no explicit songs, or only those, or only ones that stream.
        if (!keepTrack(filter, track)) continue;
        if (!tracks.has(key(track))) tracks.set(key(track), track);
      }
    }
    return [...tracks.values()]
      .map((track) => ({ track, score: songScore(track, wanted) }))
      .filter((entry) => entry.score >= GOOD_ENOUGH)
      .sort((a, b) => b.score - a.score)
      .slice(0, SHOWN)
      .map((entry) => entry.track);
    // Read once per query: the live answers take over when they come.
  }, [client, wanted, filter, needed]);
};

/** A cached title as a card: search results and browse pages as they are, titles by remote id. */
const asCard = (record: Record<string, unknown>): MangaCard | null => {
  if (typeof record.sourceId !== 'string' || typeof record.title !== 'string') return null;
  const id =
    typeof record.remoteId === 'string'
      ? record.remoteId
      : typeof record.id === 'string' && typeof record.inLibrary === 'boolean'
        ? record.id
        : null;
  if (!id) return null;
  return {
    sourceId: record.sourceId,
    id,
    title: record.title,
    coverUrl: typeof record.coverUrl === 'string' ? record.coverUrl : null,
    inLibrary: record.inLibrary === true || typeof record.titleId === 'string',
    ...(record.adult === true ? { adult: true } : {}),
  };
};

/**
 * Comics, books or videos this device has seen (M46.47: Home's rows, sources' pages, earlier
 * searches, the library) that answer `query`, best first, each once. Looked for only while
 * `needed` (some sources haven't answered): the cache is read whole.
 */
export const useSeenTitles = (medium: Medium, query: string, needed = true): MangaCard[] => {
  const client = useQueryClient();
  const { noAdult } = useSearchFilter();
  const wanted = query.trim();
  return useMemo(() => {
    if (!needed || wanted.length < 2) return NO_CARDS;
    const cards = collectFromCache(
      client,
      [medium],
      asCard,
      (card) => `${card.sourceId}:${card.id}`,
    );
    const score = medium === 'video' ? videoTitleScore : titleScore;
    return cards
      .filter((card) => !(noAdult && card.adult))
      .map((card) => ({ card, score: score(card.title, wanted) }))
      .filter((entry) => entry.score >= GOOD_ENOUGH)
      .sort((a, b) => b.score - a.score)
      .slice(0, SHOWN)
      .map((entry) => entry.card);
  }, [client, medium, wanted, noAdult, needed]);
};
