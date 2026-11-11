import type {
  MangaCard,
  MangaSource,
  OnlineTrack,
  SearchResults,
  Settings,
  SourceSearchResult,
} from '@raygun/bindings';
import type { QueryClient, QueryKey } from '@tanstack/react-query';
import { useMemo } from 'react';

import { useSettings } from '../settings/useSettings';

/**
 * What Search and Browse leave out: 18+ sources and titles while 18+ content is off (Show NSFW)
 * or while its "No 18+" pill is on (`Settings::searchFilter`, shown only while 18+ content is
 * on), and explicit songs while Explicit music is off. Rust leaves the same out where a search
 * fans out (`search_filter.rs`); this covers answers kept from before. Library titles aren't
 * filtered by the pill.
 */
export type SearchFilter = {
  /** Sources rated for adults and titles marked adult are left out. */
  noAdult: boolean;
  /** Songs marked explicit are left out (Explicit music is off). */
  noExplicit: boolean;
  /** Music sources rated 18+ are left out: the pill is on, or Explicit music is off. */
  noAdultMusic: boolean;
  /** The "No 18+" pill shows: 18+ content is on. */
  pillShown: boolean;
};

export const searchFilterOf = (settings: Settings | undefined): SearchFilter => {
  const adultOn = Boolean(settings?.showNsfw);
  const pill = adultOn && Boolean(settings?.searchFilter.noAdult);
  const noExplicit = settings ? !settings.explicitMusic : false;
  return {
    noAdult: settings ? !adultOn || pill : false,
    noExplicit,
    noAdultMusic: pill || noExplicit,
    pillShown: adultOn,
  };
};

/** The filter as the settings say now; the same object while they say the same. */
export const useSearchFilter = (): SearchFilter => {
  const settings = useSettings().data;
  const { noAdult, noExplicit, noAdultMusic, pillShown } = searchFilterOf(settings);
  return useMemo(
    () => ({ noAdult, noExplicit, noAdultMusic, pillShown }),
    [noAdult, noExplicit, noAdultMusic, pillShown],
  );
};

export const keepSource = (filter: SearchFilter, source: MangaSource) =>
  !(filter.noAdult && source.adult);

export const keepCards = (filter: SearchFilter, cards: MangaCard[]) =>
  filter.noAdult ? cards.filter((card) => !card.adult) : cards;

export const keepTracks = (filter: SearchFilter, tracks: OnlineTrack[]) =>
  filter.noExplicit ? tracks.filter((track) => !track.explicit) : tracks;

/** Artists, albums or playlists without those of music sources rated 18+. */
export const keepFromSources = <T extends { provider: { adult?: boolean } }>(
  filter: SearchFilter,
  found: T[],
): T[] => (filter.noAdultMusic ? found.filter((item) => !item.provider.adult) : found);

/** A search's songs without explicit ones, and nothing from music sources rated 18+. */
export const keepSongs = (filter: SearchFilter, results: SearchResults): SearchResults => {
  if (!filter.noExplicit && !filter.noAdultMusic) return results;
  return {
    library: results.library,
    top: keepTracks(filter, results.top),
    providers: results.providers.map((result) =>
      filter.noAdultMusic && result.provider.adult
        ? { ...result, tracks: [] }
        : { ...result, tracks: keepTracks(filter, result.tracks) },
    ),
  };
};

const sourceResultSelects = new Map<string, (result: SourceSearchResult | null) => unknown>();

/**
 * A `select` for one source's search answer under `filter`: an adult source's answer is empty,
 * adult titles are left out. One function per filter, so TanStack reruns it only when either
 * the answer or the filter changes.
 */
export const selectSourceResult = (filter: SearchFilter) => {
  const key = `${filter.noAdult}`;
  let select = sourceResultSelects.get(key);
  if (!select) {
    select = (result: SourceSearchResult | null) => {
      if (!result || !filter.noAdult) return result;
      if (result.source.adult) return { ...result, items: [] };
      const items = keepCards(filter, result.items);
      return items.length === result.items.length ? result : { ...result, items };
    };
    sourceResultSelects.set(key, select);
  }
  return select as (result: SourceSearchResult | null) => SourceSearchResult | null;
};

/** Parts of the query keys of searches and listings answered under the filter. */
const SEARCHED = new Set([
  'onlineSearch',
  'searchArtists',
  'searchAlbums',
  'searchPlaylists',
  'sourceSearch',
  'globalSearch',
  'filteredSearch',
  'search',
  'top',
  'directory',
]);

const searchedUnderFilter = (key: QueryKey) =>
  key.some((part) => typeof part === 'string' && SEARCHED.has(part));

/**
 * After the pill changes, searches kept from before are asked again in the background, so what
 * Rust left out (or now asks) follows it; what shows meanwhile is filtered here.
 */
export const refreshSearches = (client: QueryClient) =>
  client.invalidateQueries({ predicate: (query) => searchedUnderFilter(query.queryKey) });
