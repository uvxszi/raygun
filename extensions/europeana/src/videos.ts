// Films from Europeana: videos under an open licence whose institution shares an MP4 file of them
// (Europeana's search can ask for that), most of them newsreels and documentaries from European
// sound and film archives. Lists come from Europeana's search; a film's page reads its record,
// which names its licence and the institution that holds it, both shown with it. A film plays
// from the institution's own server, the file the record links to.
import { defineProvider, selectValue } from '@raygun/sdk';
import type { MangaPage, MangaSummary, SelectFilter, Video, VideoEpisode } from '@raygun/sdk';

import {
  europeana,
  extensionOf,
  firstYear,
  isOpen,
  isPage,
  isWeb,
  licenceName,
  people,
  pick,
  plainId,
  searchWords,
} from './api';
import type { LangMap } from './api';

const PAGE_SIZE = 40;
/** Europeana's search pages no further than its first 1,000 results. */
const DEEPEST = 1000;
/** Titles in English where the institution gave one, else in the film's own language. */
const LANGUAGES = ['en'];

type SearchItem = {
  id?: string;
  title?: string[];
  dcTitleLangAware?: LangMap;
  dcCreator?: string[];
  dcCreatorLangAware?: LangMap;
  rights?: string[];
  edmIsShownBy?: string[];
  edmPreview?: string[];
  language?: string[];
};
type SearchAnswer = { items?: SearchItem[]; totalResults?: number };

/** Archives name themselves as curators among the creators; they didn't make the film. */
const makers = (names: string[]) => names.filter((name) => !/\((curator|beheerder)\)$/i.test(name));

type WebResource = {
  about?: string;
  ebucoreHasMimeType?: string;
  ebucoreHeight?: number;
  webResourceEdmRights?: LangMap;
};
type Aggregation = {
  edmIsShownBy?: string;
  hasView?: string[];
  edmRights?: LangMap;
  edmDataProvider?: LangMap;
  edmProvider?: LangMap;
  webResources?: WebResource[];
};
type Proxy = {
  europeanaProxy?: boolean;
  dcTitle?: LangMap;
  dcDescription?: LangMap;
  dcCreator?: LangMap;
  dcDate?: LangMap;
  dctermsCreated?: LangMap;
  dctermsIssued?: LangMap;
  dcLanguage?: LangMap;
  year?: LangMap;
};
type RecordAnswer = {
  object?: {
    year?: string[];
    proxies?: Proxy[];
    aggregations?: Aggregation[];
    organizations?: { about?: string; prefLabel?: LangMap }[];
    concepts?: { about?: string; prefLabel?: LangMap }[];
    europeanaAggregation?: { edmPreview?: string; edmLandingPage?: string };
  };
};

const COUNTRY: SelectFilter = {
  kind: 'select',
  key: 'country',
  label: 'Country',
  options: [
    { value: 'any', label: 'Any country' },
    ...[
      'Netherlands',
      'Germany',
      'Romania',
      'Sweden',
      'Hungary',
      'Spain',
      'Finland',
      'Poland',
      'Greece',
      'Ireland',
      'France',
    ].map((country) => ({ value: country.toLowerCase(), label: country })),
  ],
  default: 'any',
};

const PROVIDER: SelectFilter = {
  kind: 'select',
  key: 'provider',
  label: 'Institution',
  options: [
    { value: 'any', label: 'Any institution' },
    // The institutions with the most open films that have an MP4.
    ...[
      'Netherlands Institute for Sound & Vision',
      'Naturalis Biodiversity Center',
      'Romanian Television',
      'Institut des Hautes Études Scientifiques',
      'Ruhr University Bochum',
      'Federal Archives of Germany',
      'Museum of Ethnography',
      'Csorba Győző Library - Pécs',
      'Federal Waterways Engineering and Research Institute',
      'German Informatics Society',
    ].map((name) => ({ value: name, label: name })),
  ],
  default: 'any',
};

// No decade filter: most of these films carry no year Europeana's search can filter by.
const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort',
  options: [
    { value: 'relevance', label: 'Relevance' },
    { value: 'added', label: 'Recently added' },
  ],
  default: 'relevance',
};

/** What every list asks for: open videos with an MP4 Europeana knows of. */
const VIDEOS = ['TYPE:VIDEO', 'MIME_TYPE:video/mp4'];

const summaryOf = (item: SearchItem): MangaSummary[] => {
  const id = item.id ? plainId(item.id) : '';
  const title =
    pick(item.dcTitleLangAware, [...LANGUAGES, ...(item.language ?? [])]) ?? pick(item.title, []);
  const media = item.edmIsShownBy?.find(isWeb);
  if (!id || !title || !isOpen(item.rights?.[0]) || !media || isPage(media)) return [];
  const languages = [...LANGUAGES, ...(item.language ?? [])];
  return [
    {
      id,
      title,
      coverUrl: item.edmPreview?.find(isWeb) ?? null,
      authors: makers(people(item.dcCreatorLangAware ?? item.dcCreator, languages)).slice(0, 3),
    },
  ];
};

const listPage = async (
  page: number,
  { words = '', refinements = [] as string[], sort = '' } = {},
): Promise<MangaPage> => {
  const asked = Math.max(1, page);
  const start = (asked - 1) * PAGE_SIZE + 1;
  if (start + PAGE_SIZE - 1 > DEEPEST) return { items: [], hasNextPage: false };
  const answer = await europeana<SearchAnswer>('search.json', {
    query: words || '*',
    qf: [...VIDEOS, ...refinements],
    reusability: 'open',
    media: true,
    rows: PAGE_SIZE,
    start,
    profile: 'minimal',
    ...(sort ? { sort } : {}),
  });
  const items = answer.items ?? [];
  const seen = new Set<string>();
  return {
    items: items.flatMap(summaryOf).filter((item) => {
      if (seen.has(item.id)) return false;
      seen.add(item.id);
      return true;
    }),
    hasNextPage:
      items.length > 0 && asked * PAGE_SIZE < Math.min(answer.totalResults ?? 0, DEEPEST),
  };
};

const record = async (id: string) => {
  const answer = await europeana<RecordAnswer>(
    `${plainId(id).split('/').map(encodeURIComponent).join('/')}.json`,
    {},
  );
  const object = answer.object;
  const aggregation = object?.aggregations?.[0];
  if (!object || !aggregation) throw new Error('This item is no longer on Europeana');
  const rights = pick(aggregation.edmRights, []);
  if (!isOpen(rights)) {
    throw new Error("This item isn't under an open licence, so it isn't offered");
  }
  return { object, aggregation, rights };
};

/** The institution an aggregation names (an organisation's address, or its name). */
const organisation = (
  value: LangMap | undefined,
  organizations: { about?: string; prefLabel?: LangMap }[] | undefined,
) => {
  const named = pick(value, []);
  if (!named) return null;
  const known = organizations?.find((found) => found.about === named);
  return known ? pick(known.prefLabel, LANGUAGES) : /^https?:\/\//.test(named) ? null : named;
};

const PLAYABLE = /^video\/(mp4|x-m4v|webm)$/i;

/** The record's video files: what it's shown by, then its other views, each with its height. */
const filesOf = (aggregation: Aggregation) => {
  const resources = new Map(
    (aggregation.webResources ?? []).map((resource) => [resource.about ?? '', resource]),
  );
  const links = [aggregation.edmIsShownBy, ...(aggregation.hasView ?? [])].filter(isWeb);
  return [...new Set(links)].flatMap((link) => {
    const resource = resources.get(link);
    const mime = resource?.ebucoreHasMimeType;
    const playable = mime
      ? PLAYABLE.test(mime)
      : ['mp4', 'm4v', 'webm'].includes(extensionOf(link));
    const ownRights = pick(resource?.webResourceEdmRights, []);
    if (!playable || isPage(link) || (ownRights && !isOpen(ownRights))) return [];
    return [{ url: link, height: resource?.ebucoreHeight ?? null }];
  });
};

/** The institution's own descriptions, in the record's order (the one its search shows first). */
const providerProxies = (proxies: Proxy[] | undefined) =>
  (proxies ?? []).filter((proxy) => !proxy.europeanaProxy);

const fromProxies = (proxies: Proxy[], field: keyof Proxy, languages: string[]) => {
  for (const proxy of proxies) {
    const found = pick(proxy[field] as LangMap | undefined, languages);
    if (found) return found;
  }
  return null;
};

/** Every text of a field in the chosen language, once each, joined into paragraphs. */
const allOf = (proxies: Proxy[], field: keyof Proxy, languages: string[]) => {
  for (const proxy of proxies) {
    const map = proxy[field] as LangMap | undefined;
    if (!map) continue;
    for (const language of [...languages, 'def', ...Object.keys(map)]) {
      const values = (map[language] ?? []).map((value) => value.trim()).filter(Boolean);
      if (values.length > 0) return [...new Set(values)].join('\n\n');
    }
  }
  return '';
};

const episodeIdOf = (id: string, index: number) => `${id}#${index}`;

const videosOf = async (episodeId: string): Promise<Video[]> => {
  const cut = episodeId.lastIndexOf('#');
  const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
  const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
  const file = filesOf((await record(id)).aggregation)[index];
  if (!file) throw new Error('This video is no longer on Europeana');
  return [
    {
      url: file.url,
      label: file.height ? `${file.height}p` : 'Video',
      ...(file.height ? { quality: file.height } : {}),
    },
  ];
};

export const videos = defineProvider({
  kind: 'video.source',
  id: 'videos',
  name: 'Europeana',

  // Europeana's own order for everything open with an MP4 (it has no counts of views).
  popular: ({ page }) => listPage(page),

  latest: ({ page }) => listPage(page, { sort: 'timestamp_created desc' }),

  search({ query: typed, page, filters }) {
    const country = selectValue(filters, COUNTRY);
    const provider = selectValue(filters, PROVIDER);
    const sort = selectValue(filters, SORT);
    const refinements: string[] = [];
    if (country !== 'any') refinements.push(`COUNTRY:${country}`);
    if (provider !== 'any') refinements.push(`DATA_PROVIDER:"${provider}"`);
    return listPage(page, {
      words: searchWords(typed),
      refinements,
      sort: sort === 'added' ? 'timestamp_created desc' : '',
    });
  },

  filters: async () => [COUNTRY, PROVIDER, SORT],

  async details({ id }) {
    const { object, aggregation, rights } = await record(id);
    const proxies = providerProxies(object.proxies);
    const languages = [
      ...LANGUAGES,
      ...proxies.flatMap((proxy) => proxy.dcLanguage?.def ?? []).map((tag) => tag.toLowerCase()),
    ];
    const holder = organisation(aggregation.edmDataProvider, object.organizations);
    const through = organisation(aggregation.edmProvider, object.organizations);
    const creators = makers(proxies.flatMap((proxy) => people(proxy.dcCreator, languages)));
    const credit = holder
      ? `Provided by ${holder}${through && through !== holder ? ` through ${through}` : ''} and Europeana.`
      : 'Provided through Europeana.';
    return {
      id,
      title: fromProxies(proxies, 'dcTitle', languages) ?? id,
      coverUrl: object.europeanaAggregation?.edmPreview ?? null,
      description: [
        allOf(proxies, 'dcDescription', languages),
        `Licence: ${licenceName(rights)} (${rights}).`,
        credit,
      ]
        .filter(Boolean)
        .join('\n\n'),
      genres: [
        ...new Set(
          (object.concepts ?? [])
            .map((concept) => pick(concept.prefLabel, LANGUAGES))
            .filter((label): label is string => Boolean(label)),
        ),
      ].slice(0, 6),
      directors: [...new Set(creators)].slice(0, 4),
      type: 'other',
      year: firstYear([
        ...proxies.flatMap((proxy) => [
          fromProxies([proxy], 'dctermsCreated', []),
          fromProxies([proxy], 'dcDate', []),
          fromProxies([proxy], 'dctermsIssued', []),
          fromProxies([proxy], 'year', []),
        ]),
        ...(object.year ?? []),
      ]),
      status: 'completed',
      url: object.europeanaAggregation?.edmLandingPage ?? `https://www.europeana.eu/item/${id}`,
    };
  },

  async episodes({ id }) {
    const files = filesOf((await record(id)).aggregation);
    if (files.length === 0) throw new Error('Europeana links no video file of this item');
    if (files.length === 1) return { episodes: [{ id: episodeIdOf(id, 0), name: 'Film' }] };
    return {
      episodes: files.map((_, index): VideoEpisode => ({
        id: episodeIdOf(id, index),
        name: `Part ${index + 1}`,
        number: index + 1,
      })),
    };
  },

  // One server, resolved at once: the institution's file, as the record links it.
  servers: async ({ episodeId }) => ({
    servers: [{ id: episodeId, name: 'Europeana', videos: await videosOf(episodeId) }],
  }),

  videos: async ({ serverId }) => ({ videos: await videosOf(serverId) }),
});
