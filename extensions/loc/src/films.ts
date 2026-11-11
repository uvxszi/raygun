// Films from the Library of Congress: its online motion pictures published more than 95 years
// ago (in the public domain in the United States), from its film collections or a search of all
// its films.
// Lists keep to what their answer shows (a film's date, its access, its video); a film's page
// reads the item's own rights statement first and refuses one that names a restriction. Each
// film plays from the Library's MP4, with its adaptive stream as a second link.
import { defineProvider, query, selectValue } from '@raygun/sdk';
import type { MangaPage, MangaSummary, SelectFilter, Video, VideoEpisode } from '@raygun/sdk';

import {
  Lru,
  contributor,
  creditLine,
  filmCutoff,
  firstYear,
  freeToUse,
  https,
  itemId,
  jukebox,
  latestYear,
  locJson,
  madeIt,
  plain,
} from './api';
import type { ItemAnswer, Listing, ListedItem, Resource } from './api';

const PAGE_SIZE = 50;
const EDISON = 'edison-company-motion-pictures-and-sound-recordings';

const COLLECTION: SelectFilter = {
  kind: 'select',
  key: 'collection',
  label: 'Collection',
  options: [
    { value: 'all', label: 'All films' },
    { value: EDISON, label: 'Edison films' },
    { value: 'america-at-work-and-leisure-1894-to-1915', label: 'America at Work, at Leisure' },
    { value: 'spanish-american-war-in-motion-pictures', label: 'Spanish-American War' },
    { value: 'mckinley-and-the-pan-american-expo-films-1901', label: 'McKinley, 1901' },
    { value: 'theodore-roosevelt-films', label: 'Theodore Roosevelt' },
    { value: 'films-of-westinghouse-works-1904', label: 'Westinghouse Works, 1904' },
    { value: 'early-films-of-new-york-1898-to-1906', label: 'Early New York' },
    { value: 'san-francisco-earthquake-and-fire-1897-to-1916', label: 'Early San Francisco' },
    { value: 'origins-of-american-animation', label: 'Early animation' },
    { value: 'variety-stage-sound-recordings-and-motion-pictures', label: 'Variety stage' },
    { value: 'national-screening-room', label: 'National Screening Room' },
  ],
  default: 'all',
};

const DECADE: SelectFilter = {
  kind: 'select',
  key: 'decade',
  label: 'Decade',
  options: [
    { value: 'any', label: 'Any time' },
    { value: '1880', label: 'Before 1900' },
    { value: '1900', label: '1900s' },
    { value: '1910', label: '1910s' },
    { value: '1920', label: '1920s' },
  ],
  default: 'any',
};

const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort',
  options: [
    { value: 'relevance', label: 'Relevance' },
    { value: 'date', label: 'Oldest first' },
    { value: 'date_desc', label: 'Newest first' },
    { value: 'title_s', label: 'Title' },
  ],
  default: 'relevance',
};

/** The years a decade option asks for, never past the public-domain cutoff. */
const datesFor = (decade: string) => {
  const cutoff = filmCutoff();
  if (decade === 'any') return `1800/${cutoff}`;
  const start = Number(decade);
  const end = Math.min(start === 1880 ? 1899 : start + 9, cutoff);
  return `${start}/${end}`;
};

const videoOf = (resource: Resource | undefined) => resource?.video ?? resource?.video_stream;

/** A listed film Raygun may offer: published before the cutoff, open, with a video. */
const publicFilm = (listed: ListedItem): boolean => {
  const year = latestYear(listed.date);
  return (
    year !== null &&
    year <= filmCutoff() &&
    listed.access_restricted !== true &&
    !jukebox(listed.partof) &&
    (listed.resources ?? []).some((resource) => Boolean(videoOf(resource)))
  );
};

const posterOf = (resources: Resource[] | undefined, pictures?: string[]) => {
  const resource = (resources ?? []).find((found) => videoOf(found));
  return https(resource?.poster) ?? https(resource?.image) ?? https(pictures?.[0]);
};

/** Who made a listed film, as the list names them (companies, camera operators, animators). */
const makersOf = (listed: ListedItem) => [
  ...new Set(
    (listed.item?.contributors ?? [])
      .filter((name): name is string => typeof name === 'string')
      .map(contributor)
      .filter(madeIt)
      .map((person) => person.name),
  ),
];

const summaryOf = (listed: ListedItem): MangaSummary[] => {
  const id = itemId(listed);
  if (!id || !listed.title || !publicFilm(listed)) return [];
  return [
    {
      id,
      title: listed.title.trim(),
      coverUrl: posterOf(listed.resources, listed.image_url),
      authors: makersOf(listed).slice(0, 3),
    },
  ];
};

const pages = new Lru<MangaPage>(60);

const listPage = async (path: string, params: Record<string, string | number>) => {
  const address = `${path}?${query({
    ...params,
    fa: 'online-format:video',
    at: 'results,pagination',
    c: PAGE_SIZE,
  })}`;
  const kept = pages.get(address);
  if (kept) return kept;
  const answer = await locJson<Listing>(address);
  const results = answer?.results ?? [];
  const found = new Map<string, MangaSummary>();
  for (const item of results.flatMap(summaryOf)) {
    if (!found.has(item.id)) found.set(item.id, item);
  }
  const page: MangaPage = {
    items: [...found.values()],
    hasNextPage: results.length > 0 && Boolean(answer?.pagination?.next),
  };
  pages.set(address, page);
  return page;
};

const items = new Lru<ItemAnswer>(300);

/** An item's own page, refused when its rights or date don't hold up. */
const film = async (id: string): Promise<ItemAnswer> => {
  const kept = items.get(id);
  if (kept) return kept;
  const answer = await locJson<ItemAnswer>(
    `/item/${encodeURIComponent(id)}/?${query({ at: 'item,resources' })}`,
  );
  if (!answer?.item) throw new Error('This film is no longer at the Library of Congress');
  const year = latestYear(answer.item.date);
  if (year === null || year > filmCutoff() || jukebox(answer.item.partof)) {
    throw new Error("This film isn't in the public domain yet, so it isn't offered");
  }
  if (answer.item.access_restricted === true || !freeToUse(answer.item)) {
    throw new Error("This item has rights restrictions, so it isn't offered");
  }
  items.set(id, answer);
  return answer;
};

/** The film's parts that play: most films are one. */
const parts = (answer: ItemAnswer) =>
  (answer.resources ?? [])
    .map((resource, index) => ({ resource, index }))
    .filter(({ resource }) => videoOf(resource));

/** An item's page names its contributors with their roles in `contributor_names`. */
const people = (answer: ItemAnswer) =>
  (answer.item?.contributor_names ?? [])
    .filter((name): name is string => typeof name === 'string')
    .map(contributor)
    .filter(madeIt);

const DIRECTING = /camera|cinematograph|director|animator|scenarist|writ/;
const ACTING = /actor|performer|cast|singer/;

const videosOf = (resource: Resource): Video[] => {
  const videos: Video[] = [];
  const mp4 = https(resource.video);
  const stream = https(resource.video_stream);
  if (mp4) {
    videos.push({
      url: mp4,
      label: resource.height ? `MP4 · ${resource.height}p` : 'MP4',
      ...(resource.height ? { quality: resource.height } : {}),
    });
  }
  if (stream) videos.push({ url: stream, label: 'Adaptive stream', format: 'hls' });
  return videos;
};

const episodeIdOf = (id: string, index: number) => `${id}#${index}`;

const resourceOf = async (episodeId: string) => {
  const cut = episodeId.lastIndexOf('#');
  const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
  const index = cut < 0 ? 0 : Number(episodeId.slice(cut + 1));
  const resource = (await film(id)).resources?.[index];
  if (!resource || !videoOf(resource)) {
    throw new Error('The Library of Congress no longer has this part of the film');
  }
  return resource;
};

export const films = defineProvider({
  kind: 'video.source',
  id: 'films',
  name: 'Library of Congress',

  // Edison's films: the Library's best-known early films, in its own order.
  popular: ({ page }) =>
    listPage(`/collections/${EDISON}/`, { dates: datesFor('any'), sp: Math.max(1, page) }),

  search({ query: words, page, filters }) {
    const collection = selectValue(filters, COLLECTION);
    const decade = selectValue(filters, DECADE);
    const sort = selectValue(filters, SORT);
    const params: Record<string, string | number> = {
      dates: datesFor(decade),
      sp: Math.max(1, page),
    };
    const typed = words.trim();
    if (typed) params.q = typed;
    if (sort !== 'relevance') params.sb = sort;
    const path = collection === 'all' ? '/film-and-videos/' : `/collections/${collection}/`;
    return listPage(path, params);
  },

  filters: async () => [COLLECTION, DECADE, SORT],

  async details({ id }) {
    const answer = await film(id);
    const item = answer.item!;
    const made = people(answer);
    const makers = made.filter((person) => !person.role || !ACTING.test(person.role));
    const credit = creditLine(item);
    const summary = plain(item.summary ?? item.description);
    const published = plain(item.created_published);
    return {
      id,
      title: item.title?.trim() || id,
      coverUrl: posterOf(answer.resources),
      description:
        [
          summary,
          published ? `Published: ${published}` : '',
          makers.length > 0
            ? `Made by: ${makers.map((person) => (person.role ? `${person.name} (${person.role})` : person.name)).join(', ')}`
            : '',
          `Public domain in the United States.${credit ? ` Credit: ${credit}.` : ''}`,
        ]
          .filter(Boolean)
          .join('\n\n') || null,
      genres: [...new Set(item.genre ?? [])].slice(0, 8),
      directors: made
        .filter((person) => person.role && DIRECTING.test(person.role))
        .map((person) => person.name),
      cast: made
        .filter((person) => person.role && ACTING.test(person.role))
        .map((person) => person.name),
      type: 'movie',
      year: firstYear(item.date),
      status: 'completed',
      url: `https://www.loc.gov/item/${encodeURIComponent(id)}/`,
    };
  },

  async episodes({ id }) {
    const found = parts(await film(id));
    if (found.length === 0) throw new Error('The Library of Congress has no video of this film');
    if (found.length === 1)
      return { episodes: [{ id: episodeIdOf(id, found[0]!.index), name: 'Film' }] };
    return {
      episodes: found.map(({ resource, index }, number): VideoEpisode => ({
        id: episodeIdOf(id, index),
        name: resource.caption?.trim() || `Part ${number + 1}`,
        number: number + 1,
      })),
    };
  },

  // One server, resolved at once: the Library serves the files itself.
  servers: async ({ episodeId }) => ({
    servers: [
      { id: episodeId, name: 'Library of Congress', videos: videosOf(await resourceOf(episodeId)) },
    ],
  }),

  videos: async ({ serverId }) => ({ videos: videosOf(await resourceOf(serverId)) }),
});
