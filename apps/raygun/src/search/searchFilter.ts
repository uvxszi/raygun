import type {
  MangaCard,
  MangaSource,
  OnlineTrack,
  SearchFilterPrefs,
  SearchResults,
  Settings,
  SourceSearchResult,
  StremioItem,
} from '@raygun/bindings';
import type { QueryClient, QueryKey } from '@tanstack/react-query';

import type { Medium } from '../manga/medium';
import { useSettings } from '../settings/useSettings';

/**
 * What Search and Browse leave out: 18+ sources and titles while 18+ content is off (Show NSFW)
 * or while its "No 18+" pill is on (`Settings::searchFilter`, shown only while 18+ content is
 * on), and explicit songs while Explicit music is off; then what each kind's Filters switch on
 * (`search/SearchFilters.tsx`). Rust leaves the same out where a search fans out
 * (`search_filter.rs`); this covers answers kept from before. Library titles aren't filtered.
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
  /** "18+ only" (videos, comics, books): titles marked adult and those of 18+ sources only. */
  adultOnly: boolean;
  /** Audio's "Explicit only" (while Explicit music is on): songs marked explicit only. */
  explicitOnly: boolean;
  /** Audio's "Playable only": songs that come with a stream of their own only. */
  playableSongs: boolean;
  /** Videos' "Playable only": Stremio titles an addon gives streams for only (Rust marks them). */
  playableVideos: boolean;
  /** Comics' "Not in library". */
  hideSavedComics: boolean;
  /** Books' "Not in library". */
  hideSavedBooks: boolean;
};

/** Every filter off, as a fresh install has them. */
export const NO_FILTERS: Required<SearchFilterPrefs> = {
  noAdult: false,
  adultOnly: false,
  explicitOnly: false,
  playableSongs: false,
  playableVideos: false,
  hideSavedComics: false,
  hideSavedBooks: false,
};

/** The filters as stored, every field given (settings kept from an older version lack some). */
export const storedFilters = (settings: Settings | undefined): Required<SearchFilterPrefs> => ({
  ...NO_FILTERS,
  ...settings?.searchFilter,
});

export const searchFilterOf = (settings: Settings | undefined): SearchFilter => {
  const adultOn = Boolean(settings?.showNsfw);
  const stored = storedFilters(settings);
  const pill = adultOn && stored.noAdult;
  const noExplicit = settings ? !settings.explicitMusic : false;
  return {
    noAdult: settings ? !adultOn || pill : false,
    noExplicit,
    noAdultMusic: pill || noExplicit,
    pillShown: adultOn,
    adultOnly: adultOn && stored.adultOnly && !stored.noAdult,
    explicitOnly: Boolean(settings?.explicitMusic) && stored.explicitOnly,
    playableSongs: stored.playableSongs,
    playableVideos: stored.playableVideos,
    hideSavedComics: stored.hideSavedComics,
    hideSavedBooks: stored.hideSavedBooks,
  };
};

const FIELDS: readonly (keyof SearchFilter)[] = [
  'noAdult',
  'noExplicit',
  'noAdultMusic',
  'pillShown',
  'adultOnly',
  'explicitOnly',
  'playableSongs',
  'playableVideos',
  'hideSavedComics',
  'hideSavedBooks',
];

/** One letter per field: filters that say the same share a key. */
const keyOf = (filter: SearchFilter) => FIELDS.map((field) => (filter[field] ? '1' : '0')).join('');

/** One object per combination, so lists filtered by it redraw only when it changes. */
const kept = new Map<string, SearchFilter>();

const stable = (filter: SearchFilter): SearchFilter => {
  const key = keyOf(filter);
  const known = kept.get(key);
  if (known) return known;
  kept.set(key, filter);
  return filter;
};

/** The filter as the settings say now; the same object while they say the same. */
export const useSearchFilter = (): SearchFilter => stable(searchFilterOf(useSettings().data));

/** The filter without Audio's song filters (Find a source lists every source's answer). */
export const withoutSongFilters = (filter: SearchFilter): SearchFilter =>
  stable({ ...filter, explicitOnly: false, playableSongs: false });

export const keepSource = (filter: SearchFilter, source: MangaSource) =>
  !(filter.noAdult && source.adult);

/** Whether "Not in library" is on for `medium`'s searches. */
export const hidesSaved = (filter: SearchFilter, medium: Medium | undefined) =>
  medium === 'manga' ? filter.hideSavedComics : medium === 'novel' && filter.hideSavedBooks;

/**
 * Whether a title stays, as Rust's `keep_cards` decides: adult ones go under "No 18+"; under
 * "18+ only" only adult ones stay, or every one of a source rated for adults (`sourceAdult`);
 * comics and books in the library go under their "Not in library".
 */
export const keepCard = (
  filter: SearchFilter,
  card: MangaCard,
  medium?: Medium,
  sourceAdult = false,
) =>
  !(filter.noAdult && card.adult) &&
  !(filter.adultOnly && !sourceAdult && !card.adult) &&
  !(hidesSaved(filter, medium) && card.inLibrary);

/** `cards` as the filter keeps them; the same array when it keeps them all. */
export const keepCards = (
  filter: SearchFilter,
  cards: MangaCard[],
  medium?: Medium,
  sourceAdult = false,
) => {
  const shown = cards.filter((card) => keepCard(filter, card, medium, sourceAdult));
  return shown.length === cards.length ? cards : shown;
};

/** A song that comes with a stream of its own (not one Raygun looks for when it plays). */
export const streamsItself = (track: OnlineTrack) =>
  track.source !== null && track.source !== undefined && track.source.kind !== 'lookup';

/** Whether a song stays: explicit ones go while Explicit music is off; then Audio's filters. */
export const keepTrack = (filter: SearchFilter, track: OnlineTrack) =>
  !(filter.noExplicit && track.explicit) &&
  !(filter.explicitOnly && !track.explicit) &&
  !(filter.playableSongs && !streamsItself(track));

export const keepTracks = (filter: SearchFilter, tracks: OnlineTrack[]) =>
  filter.noExplicit || filter.explicitOnly || filter.playableSongs
    ? tracks.filter((track) => keepTrack(filter, track))
    : tracks;

/** Artists, albums or playlists without those of music sources rated 18+. */
export const keepFromSources = <T extends { provider: { adult?: boolean } }>(
  filter: SearchFilter,
  found: T[],
): T[] => (filter.noAdultMusic ? found.filter((item) => !item.provider.adult) : found);

/** A search's songs as the filter keeps them, and nothing from music sources rated 18+. */
export const keepSongs = (filter: SearchFilter, results: SearchResults): SearchResults => {
  if (!filter.noExplicit && !filter.noAdultMusic && !filter.explicitOnly && !filter.playableSongs)
    return results;
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

/**
 * Whether a Stremio addon's rows show: one rated for adults' don't under "No 18+", the others'
 * don't under "18+ only" (their titles carry no adult mark of their own).
 */
export const keepAddon = (filter: SearchFilter, adult: boolean) =>
  adult ? !filter.noAdult : !filter.adultOnly;

/** An addon's titles under Videos' "Playable only": those Rust found an addon gives streams for. */
export const keepStremioItems = (filter: SearchFilter, items: StremioItem[]) =>
  filter.playableVideos ? items.filter((item) => item.playable) : items;

const sourceResultSelects = new Map<string, (result: SourceSearchResult | null) => unknown>();

/**
 * A `select` for one source's search answer under `filter`: an adult source's answer is empty
 * under "No 18+", titles go as [`keepCard`] says. One function per filter and medium, so
 * TanStack reruns it only when the answer or the filter changes.
 */
export const selectSourceResult = (filter: SearchFilter, medium: Medium) => {
  const key = `${medium}:${keyOf(filter)}`;
  let select = sourceResultSelects.get(key);
  if (!select) {
    select = (result: SourceSearchResult | null) => {
      if (!result) return result;
      if (filter.noAdult && result.source.adult) {
        return result.items.length === 0 ? result : { ...result, items: [] };
      }
      const items = keepCards(filter, result.items, medium, Boolean(result.source.adult));
      return items === result.items ? result : { ...result, items };
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
 * After a filter changes, searches kept from before are asked again in the background, so what
 * Rust left out (or now asks) follows it; what shows meanwhile is filtered here.
 */
export const refreshSearches = (client: QueryClient) =>
  client.invalidateQueries({ predicate: (query) => searchedUnderFilter(query.queryKey) });
