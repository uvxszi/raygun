// National Archives Catalog (catalog.archives.gov): films the US government made or holds, through
// the catalog's API, only those the National Archives marks unrestricted in both use and access
// and that it has online as MP4 files. A film's reels are its episodes.
//
// The API needs a personal key (asked for by email at Catalog_API@nara.gov), sent in the
// `x-api-key` header; the setting is required, so Raygun asks nothing while it's empty, and a key
// the API refuses says so with `needsSetup`. A refused key doesn't always come back as a 401 or
// 403: the catalogue's web front answers the API's refusal with its own web page, so a web page
// where JSON was asked for counts as a refusal too. A key allows 10,000 queries a month, so pages
// are kept in memory, and the films a list showed open without another request.
//
// The API has no sort order, so "popular" can't mean most watched. It lists the Army's library
// copies of its own films (the "Motion Picture Films from the Army Library Copy Collection"
// series), which the National Archives marks unrestricted, instead of the whole catalogue, where
// most films online are marked possibly under copyright or not yet judged and would be left out.
import { defineProvider, needsSetup, query, registerProvider, selectValue } from '@raygun/sdk';
import type { MangaPage, SelectFilter, Video } from '@raygun/sdk';

import {
  descriptionOf,
  episodesOf,
  fileUrl,
  hitsOf,
  idOf,
  isOpen,
  reelsOf,
  summaryOf,
  tidyTitle,
  yearOf,
} from './records';
import type { CatalogRecord, SearchAnswer } from './records';

const API = 'https://catalog.archives.gov/api/v2';
const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';
const PAGE_SIZE = 60;
/** Pages past the first 10,000 results need a different kind of request; lists stop there. */
const DEEPEST = 10_000;
/** The series "Motion Picture Films from the Army Library Copy Collection". */
const ARMY_LIBRARY_COPIES = 25061;

const RECORD_GROUP: SelectFilter = {
  kind: 'select',
  key: 'recordGroup',
  label: 'Record group',
  options: [
    { value: 'all', label: 'All record groups' },
    { value: '111', label: 'Army Signal Corps (111)' },
    { value: '18', label: 'Army Air Forces (18)' },
    { value: '342', label: 'Air Force (342)' },
    { value: '80', label: 'Navy, to 1947 (80)' },
    { value: '428', label: 'Navy, from 1947 (428)' },
    { value: '127', label: 'Marine Corps (127)' },
    { value: '26', label: 'Coast Guard (26)' },
    { value: '330', label: 'Secretary of Defense (330)' },
    { value: '255', label: 'NASA (255)' },
    { value: '306', label: 'Information Agency (306)' },
    { value: '208', label: 'Office of War Information (208)' },
    { value: '16', label: 'Agriculture (16)' },
    { value: '79', label: 'National Park Service (79)' },
    { value: '200', label: 'Gift collections (200)' },
  ],
  default: 'all',
};

const DECADE: SelectFilter = {
  kind: 'select',
  key: 'decade',
  label: 'Decade',
  options: [
    { value: 'any', label: 'Any time' },
    ...[1910, 1920, 1930, 1940, 1950, 1960, 1970, 1980, 1990, 2000].map((start) => ({
      value: String(start),
      label: `${start}s`,
    })),
  ],
  default: 'any',
};

/** The person's key; without one the source needs setup. */
const apiKey = (): string => {
  const key = raygun.settings.get<string>('apiKey')?.trim();
  if (!key) {
    return needsSetup(
      "The National Archives Catalog needs your own API key: ask for one by email at Catalog_API@nara.gov and enter it in this source's settings",
    );
  }
  return key;
};

const refused = () =>
  needsSetup(
    "The National Archives Catalog didn't accept the API key: check it in this source's settings",
  );

/** A memory of answers by address, the most recent few dozen. */
/** Puts `value` last in a map that keeps its `most` latest entries. */
const remember = <V>(map: Map<string, V>, key: string, value: V, most: number) => {
  map.delete(key);
  map.set(key, value);
  if (map.size > most) map.delete(map.keys().next().value!);
};

/** Pages of films shown lately, by what they asked. */
const pages = new Map<string, MangaPage>();
/** Films seen in lists or opened, so opening one again asks nothing. */
const records = new Map<string, CatalogRecord>();
const keep = (record: CatalogRecord) => {
  const id = idOf(record);
  if (id) remember(records, id, record, 400);
};

const search = async (params: Record<string, string | number | boolean>) => {
  const key = apiKey();
  const address = `${API}/records/search?${query(params)}`;
  const response = await raygun.http.fetch(address, {
    headers: { Accept: 'application/json', 'x-api-key': key, 'User-Agent': AGENT },
  });
  if (response.status === 401 || response.status === 403) refused();
  if (response.status === 429) {
    throw new Error(
      "This API key has used up the National Archives Catalog's monthly queries; they start again on the first of next month",
    );
  }
  if (!response.ok)
    throw new Error(`The National Archives Catalog answered HTTP ${response.status}`);
  if ((response.headers['content-type'] ?? '').includes('text/html')) refused();
  let answer: SearchAnswer;
  try {
    answer = await response.json<SearchAnswer>();
  } catch {
    throw new Error("The National Archives Catalog's answer couldn't be read");
  }
  if (answer.statusCode === 401 || answer.statusCode === 403) refused();
  if (answer.statusCode !== undefined && answer.statusCode !== 200) {
    const said = typeof answer.body === 'string' ? answer.body : answer.message;
    throw new Error(
      `The National Archives Catalog couldn't answer this search${said ? `: ${said}` : ''}`,
    );
  }
  const found = hitsOf(answer);
  for (const record of found.records) if (isOpen(record)) keep(record);
  return found;
};

/** A page of online films (`params` narrowing them), the open ones only. */
const filmPage = async (
  params: Record<string, string | number | boolean>,
  page: number,
): Promise<MangaPage> => {
  const asked = Math.max(1, page);
  if ((asked - 1) * PAGE_SIZE >= DEEPEST) return { items: [], hasNextPage: false };
  const all = {
    ...params,
    typeOfMaterials: 'Moving Images',
    availableOnline: true,
    limit: PAGE_SIZE,
    page: asked,
  };
  const seen = query(all);
  const kept = pages.get(seen);
  if (kept) return kept;
  const { records: found, total } = await search(all);
  const listed = {
    items: found.flatMap(summaryOf),
    hasNextPage: found.length > 0 && asked * PAGE_SIZE < Math.min(total, DEEPEST),
  };
  remember(pages, seen, listed, 60);
  return listed;
};

/** One film's record, from memory or the catalogue; refused when it isn't open. */
const film = async (id: string): Promise<CatalogRecord> => {
  let record = records.get(id);
  if (!record) {
    const { records: found } = await search({ naId: id });
    record = found.find((candidate) => idOf(candidate) === id);
    if (!record) throw new Error('This film is no longer in the National Archives Catalog');
  }
  if (!isOpen(record)) {
    throw new Error(
      'The National Archives marks this film restricted, or has no MP4 of it, so it isn’t offered',
    );
  }
  keep(record);
  return record;
};

const videosOf = async (episodeId: string): Promise<Video[]> => {
  const cut = episodeId.lastIndexOf('/');
  const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
  const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
  const reel = reelsOf(await film(id))[index];
  if (!reel?.objectUrl) throw new Error('The National Archives Catalog no longer has this reel');
  return [{ url: fileUrl(reel.objectUrl), label: 'MP4' }];
};

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'films',
    name: 'National Archives',

    popular: ({ page }) => filmPage({ q: '*', ancestorNaId: ARMY_LIBRARY_COPIES }, page),

    search({ query: words, page, filters }) {
      const group = selectValue(filters, RECORD_GROUP);
      const decade = selectValue(filters, DECADE);
      const params: Record<string, string | number> = { q: words.trim() || '*' };
      if (group !== 'all') params.recordGroupNumber = group;
      if (decade !== 'any') {
        params.startDate = decade;
        params.endDate = String(Number(decade) + 9);
      }
      return filmPage(params, page);
    },

    filters: async () => [RECORD_GROUP, DECADE],

    async details({ id }) {
      const record = await film(id);
      return {
        id,
        title: tidyTitle(record.title ?? id),
        coverUrl: null,
        description: descriptionOf(record),
        genres: [],
        type: 'movie',
        year: yearOf(record),
        status: 'completed',
        url: `https://catalog.archives.gov/id/${encodeURIComponent(id)}`,
      };
    },

    episodes: async ({ id }) => ({ episodes: episodesOf(await film(id)) }),

    // One server, resolved at once: the catalogue serves its files itself.
    servers: async ({ episodeId }) => ({
      servers: [{ id: episodeId, name: 'National Archives', videos: await videosOf(episodeId) }],
    }),

    videos: async ({ serverId }) => ({ videos: await videosOf(serverId) }),
  }),
);
