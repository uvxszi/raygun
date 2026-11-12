// NASA's Scientific Visualization Studio (svs.gsfc.nasa.gov): visualizations, animations, produced
// videos and B-roll from NASA Goddard's studios (the SVS, Goddard Media Studios and the
// Conceptual Image Lab), through the site's public search and page APIs, no key needed. The SVS
// says its content is in the public domain unless noted; some videos carry licensed music, which
// isn't, so a page whose text or credits name music is refused when it's opened and left out of
// lists from then on.
//
// Lists are the search's result cards for pages with movies (`media=movies`; the search API's
// own answers carry no pictures), eight at a time, most viewed lately, newest or best matching
// first, with the site's own filters (type, science category, studio). A page's "Video player"
// groups are its episodes, each with its files at every size and its captions.
import {
  defineProvider,
  fetchJson,
  fetchText,
  query,
  registerProvider,
  resolveUrl,
  selectValue,
} from '@raygun/sdk';
import type { MangaPage, SelectFilter, Video, VideoSubtitle } from '@raygun/sdk';

const SITE = 'https://svs.gsfc.nasa.gov';
/** The cards a result list holds: the site sends eight at a time. */
const CARDS = 8;

type Instance = {
  url?: string;
  filename?: string;
  media_type?: string;
  width?: number;
  height?: number;
};
type MediaGroup = {
  id: number;
  widget?: string;
  title?: string | null;
  caption?: string | null;
  description?: string | null;
  items?: { type?: string; instance?: Instance | null }[];
};
type Person = { name?: string };
type Page = {
  id: number;
  title?: string;
  description?: string;
  page_type?: string;
  release_date?: string;
  main_image?: Instance | null;
  credits?: { role?: string; people?: Person[] }[];
  media_groups?: MediaGroup[];
  missions?: string[];
  keywords?: string[];
  nasa_science_categories?: string[];
  studio?: string;
};

const select = (key: string, label: string, options: [string, string][]): SelectFilter => ({
  kind: 'select',
  key,
  label,
  options: options.map(([value, name]) => ({ value, label: name })),
  default: options[0]![0],
});

const SORT = select('sort', 'Sort by', [
  ['best', 'Best match'],
  ['popularity', 'Most viewed lately'],
  ['release_date', 'Newest'],
]);
const TYPE = select('type', 'Type', [
  ['all', 'All videos'],
  ['visualization', 'Visualizations'],
  ['animation', 'Animations'],
  ['produced_video', 'Produced videos'],
  ['broll', 'B-roll'],
]);
const CATEGORY = select('category', 'Science', [
  ['all', 'All sciences'],
  ['Earth', 'Earth'],
  ['Sun', 'The Sun'],
  ['Planets & Moons', 'Planets and moons'],
  ['Universe', 'The universe'],
]);
const STUDIO = select('studio', 'Studio', [
  ['all', 'All studios'],
  ['SVS', 'Scientific Visualization Studio'],
  ['GMS', 'Goddard Media Studios'],
  ['CIL', 'Conceptual Image Lab'],
]);

/** Text naming music, whose licence isn't NASA's: those pages aren't offered. */
const MUSIC =
  /\bmusic\b|\bsoundtrack\b|\bcomposer\b|universal production|killer tracks|extreme music|audio network|associated production|musicbed|artlist|epidemic sound|premiumbeat|soundstripe|audiojungle|pond5/i;

const LANGUAGES: Record<string, string> = {
  en: 'English',
  es: 'Spanish',
  fr: 'French',
  de: 'German',
  pt: 'Portuguese',
  it: 'Italian',
  ja: 'Japanese',
  zh: 'Chinese',
  ko: 'Korean',
};

const https = (url: string | undefined) => (url ?? '').replace(/^http:\/\//i, 'https://');

const plain = (html: string | null | undefined) =>
  (html ?? '')
    .replace(/<br\s*\/?>|<\/p>|<\/li>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/[ \t]+/g, ' ')
    .replace(/\s*\n\s*/g, '\n')
    .replace(/\n{2,}/g, '\n\n')
    .trim();

/** Text with the entities a card's HTML keeps written out, on one line. */
const oneLine = (html: string) => plain(html).replace(/\s+/g, ' ').trim();

/**
 * The result cards of a list (<li id="card_search_result_4720">): the page, its title, its
 * picture and its type.
 */
const cardsOf = (html: string) =>
  [...html.matchAll(/<li[^>]*id="card_search_result_(\d+)"[\s\S]*?<\/li>/g)].flatMap((card) => {
    const body = card[0];
    const id = card[1]!;
    const title = oneLine(/<h3[^>]*>([\s\S]*?)<\/h3>/.exec(body)?.[1] ?? '');
    const picture = /<img[^>]*\ssrc="([^"]+)"/.exec(body)?.[1];
    const type = oneLine(/bi-[a-z-]+[^"]*"[^>]*><\/span>([^<]*)<\/span>/.exec(body)?.[1] ?? '');
    if (!title) return [];
    return [
      {
        id,
        title,
        coverUrl: picture ? https(resolveUrl(SITE, picture)) : null,
        subtitle: type || null,
      },
    ];
  });

/** Pages refused for their music, kept across restarts (the latest 500). */
let refused: Set<string> | null = null;
const refusedPages = () => {
  if (!refused) {
    let saved: unknown = null;
    try {
      saved = raygun.storage.get<string[]>('refused');
    } catch {
      // Without a store, refusals last this run.
    }
    refused = new Set(Array.isArray(saved) ? (saved as string[]) : []);
  }
  return refused;
};
const refuse = (id: string) => {
  const pages = refusedPages();
  if (pages.has(id)) return;
  pages.add(id);
  try {
    raygun.storage.set('refused', [...pages].slice(-500));
  } catch {
    // A full store keeps the refusal for this run only.
  }
};

/** One page of a list: the site's cards for the search `params` describe, a chunk at a time. */
const listPage = async (params: Record<string, string>, page: number): Promise<MangaPage> => {
  const html = await fetchText(
    `${SITE}/api/search/results/list?${query({
      ...params,
      media: 'movies',
      sort_by_0: 'DESC',
      chunk: Math.max(1, page) - 1,
    })}`,
  );
  const cards = cardsOf(html);
  const skip = refusedPages();
  return {
    items: cards.filter((card) => !skip.has(card.id)),
    hasNextPage: cards.length >= CARDS,
  };
};

const pages = new Map<string, Promise<Page>>();
const pageOf = (id: string): Promise<Page> => {
  if (!/^\d+$/.test(id)) return Promise.reject(new Error('The SVS has no such page'));
  let page = pages.get(id);
  if (!page) {
    page = fetchJson<Page>(`${SITE}/api/${id}/`);
    page.catch(() => pages.delete(id));
    pages.set(id, page);
    if (pages.size > 60) pages.delete(pages.keys().next().value!);
  }
  return page;
};

/** Every text a page shows: its description, its groups' words and its credits' roles. */
const wordsOf = (page: Page) =>
  [
    page.description,
    ...(page.media_groups ?? []).flatMap((group) => [
      group.title,
      group.caption,
      group.description,
    ]),
    ...(page.credits ?? []).map((credit) => credit.role),
  ]
    .map((text) => plain(text))
    .join('\n');

/** A page that may be offered; refused (and remembered) when it names music. */
const openPage = async (id: string): Promise<Page> => {
  const page = await pageOf(id);
  if (MUSIC.test(wordsOf(page))) {
    refuse(id);
    throw new Error(
      "This SVS video has licensed music, which isn't in the public domain, so Raygun doesn't offer it",
    );
  }
  return page;
};

const filesOf = (group: MediaGroup) =>
  (group.items ?? []).flatMap((item) => (item.instance ? [item.instance] : []));

const isMovie = (file: Instance) =>
  file.media_type === 'Movie' && /\.(?:mp4|m4v|webm|mov)$/i.test(file.filename ?? '');

/** The groups that play: a video player with at least one movie. */
const videoGroups = (page: Page) =>
  (page.media_groups ?? []).filter(
    (group) => /video/i.test(group.widget ?? '') && filesOf(group).some(isMovie),
  );

const videosOf = (group: MediaGroup): Video[] => {
  const files = filesOf(group);
  const subtitles: VideoSubtitle[] = files
    .filter((file) => /\.(?:vtt|srt)$/i.test(file.filename ?? ''))
    // WebVTT first: each language once.
    .sort(
      (one, other) =>
        Number(/\.srt$/i.test(one.filename!)) - Number(/\.srt$/i.test(other.filename!)),
    )
    .flatMap((file) => {
      const lang = /\.([a-z]{2})(?:[_-][A-Za-z]{2})?\.(?:vtt|srt)$/i.exec(file.filename!)?.[1];
      return [{ url: https(file.url), lang: lang ?? 'en', label: LANGUAGES[lang ?? 'en'] ?? lang }];
    })
    .filter(
      (subtitle, index, all) => all.findIndex((each) => each.lang === subtitle.lang) === index,
    );
  return files
    .filter(isMovie)
    .map((file) => ({
      url: https(file.url),
      height: file.height ?? 0,
      webm: /\.webm$/i.test(file.filename ?? ''),
    }))
    .filter((file) => file.url.startsWith('https://'))
    .sort((one, other) => other.height - one.height || Number(one.webm) - Number(other.webm))
    .map((file) => ({
      url: file.url,
      label: file.height ? `${file.height}p${file.webm ? ' WebM' : ''}` : 'Video',
      ...(file.height ? { quality: file.height } : {}),
      subtitles,
    }));
};

/** An episode id: the page and its video group. */
const episodeId = (page: string, group: number) => `${page}:${group}`;

/** An episode's files, best first. */
const episodeVideos = async (id: string): Promise<Video[]> => {
  const [pageId, groupId] = id.split(':');
  const page = await openPage(pageId ?? '');
  const group = videoGroups(page).find((each) => String(each.id) === groupId);
  if (!group) throw new Error('This SVS video is no longer on its page');
  const videos = videosOf(group);
  if (videos.length === 0) throw new Error('This SVS video has no file Raygun can play');
  return videos;
};

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'videos',
    name: 'NASA Scientific Visualization Studio',

    popular: ({ page }) => listPage({ sort_by_1: 'popularity' }, page),
    latest: ({ page }) => listPage({ sort_by_1: 'release_date' }, page),

    search({ query: typed, page, filters }) {
      const words = typed.trim();
      const sort = selectValue(filters, SORT);
      const params: Record<string, string> = {
        sort_by_1: sort === 'best' ? (words ? 'relevance' : 'popularity') : sort,
      };
      if (words) params.search = words;
      const type = selectValue(filters, TYPE);
      if (type !== 'all') params.result_type = type;
      const category = selectValue(filters, CATEGORY);
      if (category !== 'all') params.nasa_science_categories = category;
      const studio = selectValue(filters, STUDIO);
      if (studio !== 'all') params.studio = studio;
      return listPage(params, page);
    },

    filters: async () => [SORT, TYPE, CATEGORY, STUDIO],

    async details({ id }) {
      const page = await openPage(id);
      const year = Number((page.release_date ?? '').slice(0, 4));
      const people = (page.credits ?? []).map(
        (credit) =>
          `${credit.role ?? 'Credit'}: ${(credit.people ?? [])
            .map((person) => person.name)
            .filter(Boolean)
            .join(', ')}`,
      );
      return {
        id,
        title: page.title?.trim() || `SVS page ${id}`,
        coverUrl: page.main_image?.url ? https(page.main_image.url) : null,
        description: [
          plain(page.description),
          people.join('\n'),
          'Public domain (NASA media): https://svs.gsfc.nasa.gov/help/',
        ]
          .filter(Boolean)
          .join('\n\n'),
        genres: [
          ...new Set([
            ...(page.nasa_science_categories ?? []),
            ...(page.missions ?? []),
            ...(page.keywords ?? []),
          ]),
        ].slice(0, 8),
        type: 'other',
        year: year >= 1800 && year <= 2200 ? year : null,
        status: 'completed',
        url: `${SITE}/${id}/`,
      };
    },

    async episodes({ id }) {
      const page = await openPage(id);
      const groups = videoGroups(page);
      if (groups.length === 0) throw new Error('This SVS page has no video Raygun can play');
      const released = Date.parse(page.release_date ?? '');
      return {
        episodes: groups.map((group, index) => ({
          id: episodeId(id, group.id),
          name:
            plain(group.title) ||
            plain(group.caption).slice(0, 80) ||
            (groups.length === 1 ? 'Video' : `Video ${index + 1}`),
          number: index + 1,
          dateUpload: Number.isFinite(released) ? released : null,
        })),
      };
    },

    // One server, resolved at once: the site serves every file itself.
    servers: async ({ episodeId: id }) => ({
      servers: [{ id, name: 'NASA SVS', videos: await episodeVideos(id) }],
    }),

    videos: async ({ serverId }) => ({ videos: await episodeVideos(serverId) }),
  }),
);
