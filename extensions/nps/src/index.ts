// National Park Service multimedia (www.nps.gov/media): the Park Service's videos through its data
// API (developer.nps.gov), as a video source, with their captions and their audio-described and
// ASL versions. Only public-domain videos are offered (the rule is in `rights.ts`): the Park
// Service's own works, credited to it, to a park or to another federal agency, or not credited at
// all, that name no one else's rights.
//
// The API needs a personal key (free, from www.nps.gov/subjects/developer/get-started.htm), sent in
// the X-Api-Key header, never in an address. The setting is required, so Raygun asks nothing while
// it's empty, and a key the API refuses says so with `needsSetup`. A key allows 1,000 requests an
// hour, so pages are kept in memory and the videos a list showed open without another request.
//
// The API has no dates or view counts: the list is its own order, filtered by park or state. Its
// audio is left out: mostly clips of a minute or two (audio descriptions of exhibits, excerpts of
// oral histories, natural sounds) that would crowd music search as songs, with no feeds to follow
// as podcasts.
import { defineProvider, needsSetup, query, registerProvider, selectValue } from '@raygun/sdk';
import type {
  MangaPage,
  MangaSummary,
  SelectFilter,
  Video,
  VideoEpisode,
  VideoSubtitle,
} from '@raygun/sdk';

import { PARK_CODES, PARKS, STATES } from './parks';
import { credited, describesNoOtherRights } from './rights';

const API = 'https://developer.nps.gov/api/v1';
const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';
const PAGE_SIZE = 50;

type Version = {
  url?: string;
  fileType?: string;
  heightPixels?: number | null;
};
type CaptionFile = { url?: string; language?: string; fileType?: string };
type RelatedPark = { parkCode?: string; fullName?: string };
/** A video as `/multimedia/videos` lists it. */
type NpsVideo = {
  id?: string;
  permalinkUrl?: string;
  title?: string;
  description?: string;
  splashImage?: { url?: string } | null;
  relatedParks?: RelatedPark[];
  tags?: string[];
  durationMs?: number | null;
  credit?: string;
  audioDescriptionUrl?: string;
  aslVideoUrl?: string;
  captionFiles?: CaptionFile[];
  versions?: Version[];
};
type Answer = {
  total?: string | number;
  data?: NpsVideo[];
  error?: { code?: string; message?: string };
};

const PARK: SelectFilter = {
  kind: 'select',
  key: 'park',
  label: 'Park',
  options: [
    { value: 'all', label: 'All parks' },
    ...PARKS.map(([code, name]) => ({ value: code, label: name })),
  ],
  default: 'all',
};

const STATE: SelectFilter = {
  kind: 'select',
  key: 'state',
  label: 'State',
  options: [
    { value: 'all', label: 'All states and territories' },
    ...STATES.map(([code, name]) => ({ value: code, label: name })),
  ],
  default: 'all',
};

/** Caption languages as the API names them, with their codes. */
const LANGUAGES: Record<string, string> = {
  english: 'en',
  spanish: 'es',
  french: 'fr',
  german: 'de',
  italian: 'it',
  portuguese: 'pt',
  russian: 'ru',
  japanese: 'ja',
  korean: 'ko',
  mandarin: 'zh',
  chinese: 'zh',
  cantonese: 'yue',
  vietnamese: 'vi',
  tagalog: 'tl',
  arabic: 'ar',
  hawaiian: 'haw',
  samoan: 'sm',
  chamorro: 'ch',
};

/** The person's key; without one the source needs setup. */
const apiKey = (): string => {
  const key = raygun.settings.get<string>('apiKey')?.trim();
  if (!key) {
    return needsSetup(
      "The National Park Service's API needs your own free key: get one at nps.gov/subjects/developer/get-started.htm and enter it in this source's settings",
    );
  }
  return key;
};

const refused = () =>
  needsSetup(
    "The National Park Service's API didn't accept the API key: check it in this source's settings",
  );

/** Puts `value` last in a map that keeps its `most` latest entries. */
const remember = <V>(map: Map<string, V>, key: string, value: V, most: number) => {
  map.delete(key);
  map.set(key, value);
  if (map.size > most) map.delete(map.keys().next().value!);
};

/** Pages of videos shown lately, by what they asked. */
const pages = new Map<string, MangaPage>();
/** Videos seen in lists or opened, by id, so opening one asks nothing more. */
const videos = new Map<string, NpsVideo>();

const ask = async (params: Record<string, string | number>): Promise<Answer> => {
  const key = apiKey();
  const response = await raygun.http.fetch(`${API}/multimedia/videos?${query(params)}`, {
    headers: { Accept: 'application/json', 'X-Api-Key': key, 'User-Agent': AGENT },
  });
  // Refusals come as JSON too: `{ "error": { "code": "API_KEY_INVALID", ... } }`.
  const answer = await response.json<Answer>().catch(() => null);
  const code = answer?.error?.code ?? '';
  if (response.status === 401 || response.status === 403 || code.startsWith('API_KEY_')) {
    refused();
  }
  if (response.status === 429 || code === 'OVER_RATE_LIMIT') {
    throw new Error(
      "This API key has made all the requests the National Park Service's API allows for now (a personal key makes 1,000 an hour); try again later",
    );
  }
  if (!response.ok) {
    throw new Error(`The National Park Service's API answered HTTP ${response.status}`);
  }
  if (!answer) throw new Error("The National Park Service's API answer couldn't be read");
  return answer;
};

const isFile = (url: string | undefined): url is string =>
  /^https?:\/\/\S+\.mp4$/i.test((url ?? '').trim());

/** A video's MP4 files, each once. */
const filesOf = (video: NpsVideo): Version[] => {
  const seen = new Set<string>();
  return (video.versions ?? []).filter((version) => {
    const url = version.url?.trim() ?? '';
    if (!isFile(url) || seen.has(url)) return false;
    seen.add(url);
    return true;
  });
};

/** Whether Raygun offers a video: a public-domain work with a file to play. */
const offered = (video: NpsVideo): boolean =>
  Boolean(video.id) &&
  filesOf(video).length > 0 &&
  credited(video.credit) &&
  describesNoOtherRights(video.description ?? '');

/** "(Video) Explore & Exercise on the Rim Trail (03:29)" → "Explore & Exercise on the Rim Trail". */
const tidyTitle = (title: string) =>
  title
    .replace(/^\s*\(video\)\s*/i, '')
    .replace(/\s*\(\d{1,2}:\d{2}(?::\d{2})?\)\s*$/, '')
    .replace(/\s+/g, ' ')
    .trim();

/**
 * The video's picture: the one the API gives, else for an older video the `_splash.jpg` the site
 * keeps beside its files ("…/yell-ElkBugle21_480x270.mp4" → "…/yell-ElkBugle21_splash.jpg").
 */
const coverOf = (video: NpsVideo): string | null => {
  const splash = video.splashImage?.url?.trim() ?? '';
  if (/^https?:\/\//i.test(splash)) return splash;
  const legacy = filesOf(video)
    .map((version) => version.url!.trim())
    .find((url) => /\/legacy\/.+_\d+x\d+\.mp4$/i.test(url));
  return legacy ? legacy.replace(/_\d+x\d+\.mp4$/i, '_splash.jpg') : null;
};

const summaryOf = (video: NpsVideo): MangaSummary[] =>
  offered(video)
    ? [
        {
          id: video.id!,
          title: tidyTitle(video.title ?? '') || video.id!,
          coverUrl: coverOf(video),
        },
      ]
    : [];

const keep = (video: NpsVideo) => {
  if (video.id) remember(videos, video.id.toUpperCase(), video, 400);
};

/** A page of the library's videos (`params` narrowing them), the public-domain ones only. */
const videoPage = async (params: Record<string, string>, page: number): Promise<MangaPage> => {
  const asked = Math.max(1, page);
  const all = { ...params, limit: PAGE_SIZE, start: (asked - 1) * PAGE_SIZE };
  const seen = query(all);
  const kept = pages.get(seen);
  if (kept) return kept;
  const answer = await ask(all);
  const found = answer.data ?? [];
  for (const video of found) if (offered(video)) keep(video);
  const total = Number(answer.total ?? 0);
  const listed = {
    items: found.flatMap(summaryOf),
    hasNextPage: found.length > 0 && asked * PAGE_SIZE < total,
  };
  remember(pages, seen, listed, 60);
  return listed;
};

/** One video, from memory or the API (searched by its id); refused when it isn't offered. */
const videoOf = async (id: string): Promise<NpsVideo> => {
  let video = videos.get(id.toUpperCase());
  if (!video) {
    const answer = await ask({ q: id, limit: 10 });
    video = (answer.data ?? []).find(
      (candidate) => candidate.id?.toUpperCase() === id.toUpperCase(),
    );
    if (!video) throw new Error("This video is no longer in the National Park Service's library");
  }
  if (!offered(video)) {
    throw new Error(
      "This video names someone other than the National Park Service or a federal agency, so Raygun doesn't offer it",
    );
  }
  keep(video);
  return video;
};

/** A file's height from its name ("_1280x720.mp4", "720p.mp4"); older entries' own are wrong. */
const heightOf = (version: Version): number | undefined => {
  const url = version.url?.trim() ?? '';
  const sized = /_\d{2,4}x(\d{2,4})\.mp4$/i.exec(url);
  if (sized) return Number(sized[1]);
  const named = /(\d{3,4})p\.mp4$/i.exec(url);
  if (named) return Number(named[1]);
  const height = Number(version.heightPixels);
  return Number.isFinite(height) && height > 0 ? Math.round(height) : undefined;
};

const subtitlesOf = (video: NpsVideo): VideoSubtitle[] =>
  (video.captionFiles ?? []).flatMap((file) => {
    const url = file.url?.trim() ?? '';
    const kind = `${file.fileType ?? ''} ${url}`;
    if (!/^https?:\/\//i.test(url) || !/vtt|srt/i.test(kind)) return [];
    const language = (file.language ?? '').trim().toLowerCase();
    const label = language ? language.charAt(0).toUpperCase() + language.slice(1) : 'Captions';
    const lang = LANGUAGES[language];
    return [lang ? { url, lang, label } : { url, label }];
  });

/** A clock for a length in milliseconds: "4:05", "1:02:03". */
const clock = (ms: number) => {
  const seconds = Math.round(ms / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = String(seconds % 60).padStart(2, '0');
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
};

const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;|&rsquo;/g, '’')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/[^\S\n]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

/** Tags too general to say anything about a video. */
const GENERIC_TAGS = new Set([
  'national',
  'park',
  'parks',
  'national park',
  'national parks',
  'national park service',
  'video',
  'videos',
  'b-roll',
  'broll',
]);

/** The video's tags, each once, without park codes and abbreviations ("bisc", "fl", "BNP"). */
const genresOf = (video: NpsVideo) => {
  const seen = new Set<string>();
  return (video.tags ?? [])
    .map((tag) => tag.replace(/\s+/g, ' ').trim())
    .filter((tag) => {
      const key = tag.toLowerCase();
      if (key.length <= 3 || key.length > 40 || PARK_CODES.has(key) || GENERIC_TAGS.has(key)) {
        return false;
      }
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .slice(0, 8);
};

const descriptionOf = (video: NpsVideo): string => {
  const parks = (video.relatedParks ?? []).flatMap((park) =>
    park.fullName?.trim() ? [park.fullName.trim()] : [],
  );
  const length = Number(video.durationMs);
  const facts = [
    parks.join(', '),
    Number.isFinite(length) && length > 0 ? clock(length) : '',
  ].filter(Boolean);
  const credit = video.credit?.trim() || 'National Park Service';
  return [
    plainText(video.description ?? ''),
    facts.join(' · '),
    `Credit: ${credit}. Public domain in the United States: a work of the US government (17 U.S.C. § 105).`,
  ]
    .filter(Boolean)
    .join('\n\n');
};

/** The episode ids: the video's own id, then `/described` or `/asl` for those versions. */
const VERSIONS = {
  described: {
    field: 'audioDescriptionUrl',
    name: 'With audio description',
    label: 'Audio description',
  },
  asl: { field: 'aslVideoUrl', name: 'With ASL interpretation', label: 'ASL' },
} as const;

const videosOf = async (episodeId: string): Promise<Video[]> => {
  const cut = episodeId.indexOf('/');
  const id = cut < 0 ? episodeId : episodeId.slice(0, cut);
  const kind = cut < 0 ? null : episodeId.slice(cut + 1);
  const video = await videoOf(id);
  if (kind === 'described' || kind === 'asl') {
    const version = VERSIONS[kind];
    const url = video[version.field]?.trim();
    if (!isFile(url)) {
      throw new Error("The National Park Service's library no longer has this version");
    }
    // Captions follow the plain video's timing, which these versions don't keep.
    return [{ url, label: version.label }];
  }
  const subtitles = subtitlesOf(video);
  const files = filesOf(video)
    .map((version) => ({ url: version.url!.trim(), height: heightOf(version) }))
    .sort((a, b) => (b.height ?? 0) - (a.height ?? 0));
  if (files.length === 0) {
    throw new Error("The National Park Service's library has no MP4 of this video");
  }
  return files.map(({ url, height }) => ({
    url,
    label: height ? `${height}p` : 'MP4',
    quality: height,
    subtitles,
  }));
};

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'videos',
    name: 'National Park Service',

    popular: ({ page }) => videoPage({}, page),

    search({ query: words, page, filters }) {
      const park = selectValue(filters, PARK);
      const state = selectValue(filters, STATE);
      const params: Record<string, string> = {};
      const text = words.trim();
      if (text) params.q = text;
      if (park !== 'all') params.parkCode = park;
      if (state !== 'all') params.stateCode = state;
      return videoPage(params, page);
    },

    filters: async () => [PARK, STATE],

    async details({ id }) {
      const video = await videoOf(id);
      return {
        id,
        title: tidyTitle(video.title ?? '') || id,
        coverUrl: coverOf(video),
        description: descriptionOf(video),
        genres: genresOf(video),
        type: 'other',
        year: null,
        status: 'completed',
        url:
          video.permalinkUrl?.trim() ||
          `https://www.nps.gov/media/video/view.htm?id=${encodeURIComponent(id)}`,
      };
    },

    async episodes({ id }) {
      const video = await videoOf(id);
      const episodes: VideoEpisode[] = [{ id, name: 'Video' }];
      for (const [kind, version] of Object.entries(VERSIONS)) {
        if (isFile(video[version.field]))
          episodes.push({ id: `${id}/${kind}`, name: version.name });
      }
      return { episodes };
    },

    // One server, resolved at once: the Park Service serves its files itself.
    servers: async ({ episodeId }) => ({
      servers: [
        { id: episodeId, name: 'National Park Service', videos: await videosOf(episodeId) },
      ],
    }),

    videos: async ({ serverId }) => ({ videos: await videosOf(serverId) }),
  }),
);
