import type {
  MangaCard,
  MangaSource,
  OnlineTrack,
  SearchResults,
  Settings,
  SourceSearchResult,
} from '@raygun/bindings';
import { useMemo } from 'react';

import { useSettings } from '../settings/useSettings';

/**
 * What Search leaves out: its own "No 18+" and "No explicit" switches (`Settings::searchFilter`),
 * or what Show NSFW and Explicit music already hide (`adultForced`, `explicitForced`). Rust leaves
 * the same out where a search fans out (`search_filter.rs`); this covers answers kept from before.
 */
export type SearchFilter = {
  /** Sources rated for adults and titles marked adult are left out. */
  noAdult: boolean;
  /** Songs marked explicit are left out. */
  noExplicit: boolean;
  /** Music sources rated 18+ are left out: either switch, an 18+ music source being explicit. */
  noAdultMusic: boolean;
  adultForced: boolean;
  explicitForced: boolean;
};

export const searchFilterOf = (settings: Settings | undefined): SearchFilter => {
  const own = settings?.searchFilter;
  const adultForced = settings ? !settings.showNsfw : false;
  const explicitForced = settings ? !settings.explicitMusic : false;
  const noExplicit = Boolean(own?.noExplicit) || explicitForced;
  return {
    noAdult: Boolean(own?.noAdult) || adultForced,
    noExplicit,
    noAdultMusic: Boolean(own?.noAdult) || noExplicit,
    adultForced,
    explicitForced,
  };
};

/** The filter as the settings say now; the same object while they say the same. */
export const useSearchFilter = (): SearchFilter => {
  const settings = useSettings().data;
  const { noAdult, noExplicit, noAdultMusic, adultForced, explicitForced } =
    searchFilterOf(settings);
  return useMemo(
    () => ({ noAdult, noExplicit, noAdultMusic, adultForced, explicitForced }),
    [noAdult, noExplicit, noAdultMusic, adultForced, explicitForced],
  );
};

export const keepSource = (filter: SearchFilter, source: MangaSource) =>
  !(filter.noAdult && source.adult);

export const keepCards = (filter: SearchFilter, cards: MangaCard[]) =>
  filter.noAdult ? cards.filter((card) => !card.adult) : cards;

export const keepTracks = (filter: SearchFilter, tracks: OnlineTrack[]) =>
  filter.noExplicit ? tracks.filter((track) => !track.explicit) : tracks;

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
