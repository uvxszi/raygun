// PeerTube: videos from PeerTube, the federated video platform. Sepia Search (Framasoft's index of
// the instances listed on joinpeertube.org, https://sepiasearch.org) finds them; each video's own
// instance answers its details, files, captions and comments through PeerTube's REST API.
//
// Only videos their uploaders publish under a Creative Commons licence or dedicate to the public
// domain are listed or played: searches ask Sepia Search for those licences alone, and the instance's
// answer is checked again when a video opens. Live streams are left out, and videos marked sensitive
// show only while NSFW content is on. A video is a title with one episode; its instance is the one
// server, with the HLS playlist and the files at each height.
import {
  defineProvider,
  fetchJson,
  query,
  registerProvider,
  resolveUrl,
  selectValue,
} from '@raygun/sdk';
import type {
  FilterValues,
  MangaPage,
  SelectFilter,
  Video,
  VideoComment,
  VideoDetails,
  VideoSubtitle,
} from '@raygun/sdk';

const SEARCH = 'https://sepiasearch.org/api/v1/search/videos';
const COUNT = 24;
const COMMENTS = 20;

/** PeerTube's licence ids that are open: the six Creative Commons licences and CC0. */
const LICENCES: Record<number, string> = {
  1: 'Creative Commons Attribution (CC BY)',
  2: 'Creative Commons Attribution-ShareAlike (CC BY-SA)',
  3: 'Creative Commons Attribution-NoDerivatives (CC BY-ND)',
  4: 'Creative Commons Attribution-NonCommercial (CC BY-NC)',
  5: 'Creative Commons Attribution-NonCommercial-ShareAlike (CC BY-NC-SA)',
  6: 'Creative Commons Attribution-NonCommercial-NoDerivatives (CC BY-NC-ND)',
  7: 'Public domain dedication (CC0)',
};
const LICENCE: SelectFilter = {
  kind: 'select',
  key: 'licence',
  label: 'Licence',
  default: 'open',
  options: [
    { value: 'open', label: 'Any open licence' },
    { value: 'commercial', label: 'Allows commercial use' },
    { value: 'changes', label: 'Allows changes' },
    { value: 'public', label: 'Public domain' },
  ],
};
const LICENCE_IDS: Record<string, number[]> = {
  open: [1, 2, 3, 4, 5, 6, 7],
  commercial: [1, 2, 3, 7],
  changes: [1, 2, 4, 5, 7],
  public: [7],
};
const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort by',
  default: '-match',
  options: [
    { value: '-match', label: 'Best match' },
    { value: '-hot', label: 'Trending' },
    { value: '-publishedAt', label: 'Newest' },
    { value: '-views', label: 'Most viewed' },
    { value: '-likes', label: 'Most liked' },
  ],
};
/** PeerTube's categories, by their ids. */
const CATEGORY: SelectFilter = {
  kind: 'select',
  key: 'category',
  label: 'Category',
  default: 'any',
  options: [
    { value: 'any', label: 'Any' },
    { value: '1', label: 'Music' },
    { value: '2', label: 'Films' },
    { value: '3', label: 'Vehicles' },
    { value: '4', label: 'Art' },
    { value: '5', label: 'Sports' },
    { value: '6', label: 'Travels' },
    { value: '7', label: 'Gaming' },
    { value: '8', label: 'People' },
    { value: '9', label: 'Comedy' },
    { value: '10', label: 'Entertainment' },
    { value: '11', label: 'News & Politics' },
    { value: '12', label: 'How To' },
    { value: '13', label: 'Education' },
    { value: '14', label: 'Activism' },
    { value: '15', label: 'Science & Technology' },
    { value: '16', label: 'Animals' },
    { value: '17', label: 'Kids' },
    { value: '18', label: 'Food' },
  ],
};
const LANGUAGE: SelectFilter = {
  kind: 'select',
  key: 'language',
  label: 'Language',
  default: 'any',
  options: [
    { value: 'any', label: 'Any' },
    { value: 'en', label: 'English' },
    { value: 'fr', label: 'French' },
    { value: 'de', label: 'German' },
    { value: 'es', label: 'Spanish' },
    { value: 'it', label: 'Italian' },
    { value: 'pt', label: 'Portuguese' },
    { value: 'nl', label: 'Dutch' },
    { value: 'ca', label: 'Catalan' },
    { value: 'eo', label: 'Esperanto' },
    { value: 'pl', label: 'Polish' },
    { value: 'cs', label: 'Czech' },
    { value: 'sv', label: 'Swedish' },
    { value: 'fi', label: 'Finnish' },
    { value: 'ru', label: 'Russian' },
    { value: 'ja', label: 'Japanese' },
    { value: 'zh', label: 'Chinese' },
    { value: 'ar', label: 'Arabic' },
  ],
};
const DURATION: SelectFilter = {
  kind: 'select',
  key: 'duration',
  label: 'Length',
  default: 'any',
  options: [
    { value: 'any', label: 'Any' },
    { value: 'short', label: 'Under 4 minutes' },
    { value: 'medium', label: '4 to 20 minutes' },
    { value: 'long', label: 'Over 20 minutes' },
  ],
};
/** Seconds, for Sepia Search's `durationMin` and `durationMax`. */
const DURATIONS: Record<string, { durationMin?: number; durationMax?: number }> = {
  short: { durationMax: 239 },
  medium: { durationMin: 240, durationMax: 1200 },
  long: { durationMin: 1201 },
};

type Named<T> = { id: T; label?: string };
type Avatar = { path?: string; fileUrl?: string; width?: number };
type Actor = {
  name?: string;
  displayName?: string;
  host?: string;
  avatars?: Avatar[];
};
/** A video as Sepia Search lists it. */
type Found = {
  uuid?: string;
  url?: string;
  name?: string;
  isLive?: boolean;
  nsfw?: boolean;
  licence?: Named<number | null>;
  thumbnailUrl?: string;
  previewUrl?: string;
  channel?: Actor;
};
type SearchAnswer = { total?: number; data?: Found[] };
type VideoFile = {
  resolution?: Named<number>;
  fileUrl?: string;
  hasVideo?: boolean;
};
type Playlist = { type?: number; playlistUrl?: string; files?: VideoFile[] };
/** A video as its instance answers it. */
type Item = {
  uuid: string;
  name?: string;
  description?: string | null;
  truncatedDescription?: string | null;
  duration?: number;
  isLive?: boolean;
  nsfw?: boolean;
  publishedAt?: string;
  originallyPublishedAt?: string | null;
  privacy?: Named<number>;
  licence?: Named<number | null>;
  category?: Named<number | null>;
  language?: Named<string | null>;
  tags?: string[];
  previewPath?: string;
  thumbnailPath?: string;
  channel?: Actor;
  account?: Actor;
  files?: VideoFile[];
  streamingPlaylists?: Playlist[];
};
type Caption = {
  language?: Named<string>;
  captionPath?: string;
  fileUrl?: string;
  automaticallyGenerated?: boolean;
};
type Thread = {
  text?: string;
  isDeleted?: boolean;
  createdAt?: string;
  totalReplies?: number;
  account?: Actor;
};

const MONTHS = [
  'January',
  'February',
  'March',
  'April',
  'May',
  'June',
  'July',
  'August',
  'September',
  'October',
  'November',
  'December',
];

/** A video's address on its instance, which is also its id here. */
const WATCH =
  /^(https?:\/\/[^/?#]+)\/(?:videos\/watch|w)\/([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\/?$/i;

const parseId = (id: string) => {
  const parts = WATCH.exec(id.trim());
  if (!parts) throw new Error('PeerTube has no such video');
  return { origin: parts[1]!, uuid: parts[2]!.toLowerCase() };
};

const idOf = (origin: string, uuid: string) => `${origin}/videos/watch/${uuid}`;

const isOpen = (licence: Named<number | null> | undefined) =>
  typeof licence?.id === 'number' && licence.id in LICENCES;

const decodeEntities = (text: string) =>
  text
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(Number.parseInt(code, 16)),
    )
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&');

/** A comment's HTML as plain text. */
const plainText = (html: string) =>
  decodeEntities(
    html
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>\s*<p[^>]*>/gi, '\n\n')
      .replace(/<[^>]*>/g, ''),
  )
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

/** A description's Markdown as plain text: links keep their words, pictures go. */
const fromMarkdown = (text: string) =>
  text
    .replace(/\r\n?/g, '\n')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/(\*\*|__)(.+?)\1/g, '$2')
    .replace(/(^|[\s(])[*_]([^*_\n]+)[*_](?=$|[\s).,;:!?])/gm, '$1$2')
    .replace(/^#{1,6}\s+/gm, '')
    .replace(/^>\s?/gm, '')
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

const clock = (seconds: number) => {
  const whole = Math.max(0, Math.round(seconds));
  const hours = Math.floor(whole / 3600);
  const minutes = Math.floor((whole % 3600) / 60);
  const rest = String(whole % 60).padStart(2, '0');
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
};

const dayOf = (iso: string | null | undefined) => {
  const time = iso ? Date.parse(iso) : NaN;
  if (!Number.isFinite(time)) return null;
  const date = new Date(time);
  return {
    time,
    text: `${date.getUTCDate()} ${MONTHS[date.getUTCMonth()]} ${date.getUTCFullYear()}`,
    iso: date.toISOString().slice(0, 10),
    year: date.getUTCFullYear(),
  };
};

/** Who published a video: its channel, and the account behind it when that's someone else. */
const byline = (item: Item) => {
  const channel = item.channel?.displayName || item.channel?.name;
  const account = item.account?.displayName || item.account?.name;
  const handle =
    item.channel?.name && item.channel.host ? `${item.channel.name}@${item.channel.host}` : null;
  const named = [channel, handle ? `(${handle})` : null].filter(Boolean).join(' ');
  return [named, account && account !== channel ? `by ${account}` : null]
    .filter(Boolean)
    .join(', ');
};

const absolute = (origin: string, path: string | undefined) =>
  path ? resolveUrl(`${origin}/`, path) : null;

// ---- Lists ----

/** A page of Sepia Search's answer: open licences only, no lives, sensitive videos as asked. */
const searchPage = async (
  params: Record<string, string | number | string[] | number[] | undefined>,
  page: number,
): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const showNsfw = raygun.content.showNsfw();
  const answer = await fetchJson<SearchAnswer>(
    `${SEARCH}?${query({
      ...params,
      start: (at - 1) * COUNT,
      count: COUNT,
      nsfw: showNsfw ? 'both' : 'false',
      isLive: 'false',
    })}`,
  );
  const items = (answer.data ?? []).flatMap((video) => {
    const origin = /^(https?:\/\/[^/?#]+)\//.exec(video.url ?? '')?.[1];
    const uuid = video.uuid?.toLowerCase();
    if (!origin || !uuid || !video.name || video.isLive || !isOpen(video.licence)) return [];
    if (video.nsfw && !showNsfw) return [];
    const channel = video.channel?.displayName || video.channel?.name;
    return [
      {
        id: idOf(origin, uuid),
        title: video.name,
        coverUrl: video.previewUrl ?? video.thumbnailUrl ?? null,
        authors: channel ? [channel] : [],
        nsfw: video.nsfw === true,
      },
    ];
  });
  return { items, hasNextPage: at * COUNT < (answer.total ?? 0) };
};

const openLicences = LICENCE_IDS.open!;

const search = (words: string, page: number, chosen?: FilterValues) => {
  const category = selectValue(chosen, CATEGORY);
  const language = selectValue(chosen, LANGUAGE);
  const trimmed = words.trim();
  return searchPage(
    {
      search: trimmed || undefined,
      sort: selectValue(chosen, SORT),
      licenceOneOf: LICENCE_IDS[selectValue(chosen, LICENCE)] ?? openLicences,
      categoryOneOf: category === 'any' ? undefined : category,
      languageOneOf: language === 'any' ? undefined : language,
      ...(DURATIONS[selectValue(chosen, DURATION)] ?? {}),
    },
    page,
  );
};

// ---- A video ----

// A video's answer serves details, episodes and servers; keep the last few.
const items = new Map<string, Promise<Item>>();

const fetchItem = (id: string): Promise<Item> => {
  let item = items.get(id);
  if (!item) {
    const { origin, uuid } = parseId(id);
    item = (async () => {
      const address = `${origin}/api/v1/videos/${uuid}`;
      const response = await raygun.http.fetch(address, {
        headers: { Accept: 'application/json' },
      });
      if (response.status === 404) throw new Error('This video is no longer on its instance');
      if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
      return response.json<Item>();
    })();
    item.catch(() => items.delete(id));
    items.set(id, item);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return item;
};

/** A video that may be shown here: public, open, not live, sensitive only as asked. */
const video = async (id: string) => {
  const item = await fetchItem(id);
  if (item.privacy && item.privacy.id !== 1) throw new Error('This video is no longer public');
  if (!isOpen(item.licence)) {
    throw new Error(
      "This video's uploader doesn't publish it under an open licence, so Raygun doesn't play it",
    );
  }
  if (item.isLive)
    throw new Error("This is a live stream, and PeerTube's lives aren't listed here");
  if (item.nsfw && !raygun.content.showNsfw()) {
    throw new Error('This video is marked sensitive; it shows while NSFW content is on');
  }
  return item;
};

const details = async (id: string): Promise<VideoDetails> => {
  const item = await video(id);
  const { origin } = parseId(id);
  const published = dayOf(item.originallyPublishedAt) ?? dayOf(item.publishedAt);
  const facts = [
    published ? `Published ${published.text}` : null,
    item.duration ? clock(item.duration) : null,
    item.category?.label ?? null,
    item.language?.id ? item.language.label : null,
  ].filter(Boolean);
  const host = origin.replace(/^https?:\/\//, '');
  const text = fromMarkdown(item.description ?? item.truncatedDescription ?? '');
  const who = byline(item);
  return {
    id,
    title: item.name ?? 'PeerTube video',
    coverUrl: absolute(origin, item.previewPath ?? item.thumbnailPath),
    description: [
      text,
      facts.join(' · '),
      who ? `Published on ${host} by ${who}.` : `Published on ${host}.`,
      `Licence: ${LICENCES[item.licence!.id as number]}, as its uploader set it.`,
    ]
      .filter(Boolean)
      .join('\n\n'),
    genres: [...new Set([item.category?.label ?? '', ...(item.tags ?? [])].filter(Boolean))].slice(
      0,
      8,
    ),
    type: item.category?.id === 2 ? 'movie' : 'other',
    year: published && published.year >= 1800 && published.year <= 2200 ? published.year : null,
    status: 'completed',
    url: id,
  };
};

const captions = async (origin: string, uuid: string): Promise<VideoSubtitle[]> => {
  try {
    const answer = await fetchJson<{ data?: Caption[] }>(
      `${origin}/api/v1/videos/${uuid}/captions`,
    );
    return (answer.data ?? []).flatMap((caption) => {
      const url = caption.fileUrl ?? absolute(origin, caption.captionPath);
      if (!url || !/^https?:\/\//.test(url)) return [];
      const name = caption.language?.label ?? caption.language?.id ?? 'Captions';
      return [
        {
          url,
          lang: caption.language?.id,
          label: caption.automaticallyGenerated ? `${name} (automatic)` : name,
        },
      ];
    });
  } catch {
    // A video plays without its captions.
    return [];
  }
};

/** A video's links: its HLS playlist, then its files from the tallest down. */
const links = async (id: string): Promise<Video[]> => {
  const item = await video(id);
  const { origin, uuid } = parseId(id);
  const subtitles = (await captions(origin, uuid)).slice(0, 50);
  const heights = (files: VideoFile[] | undefined) =>
    (files ?? [])
      .map((file) => file.resolution?.id ?? 0)
      .filter((height) => Number.isInteger(height) && height > 0);
  const found: Video[] = [];
  for (const playlist of item.streamingPlaylists ?? []) {
    if (playlist.type !== 1 || !playlist.playlistUrl?.startsWith('http')) continue;
    const tallest = Math.max(0, ...heights(playlist.files));
    found.push({
      url: playlist.playlistUrl,
      format: 'hls',
      label: tallest > 0 ? `${tallest}p (HLS)` : 'HLS',
      quality: tallest > 0 ? tallest : undefined,
      subtitles,
    });
  }
  const files = (item.files ?? [])
    .filter((file) => file.fileUrl?.startsWith('http'))
    .sort((a, b) => (b.resolution?.id ?? 0) - (a.resolution?.id ?? 0));
  for (const file of files) {
    const height = file.resolution?.id ?? 0;
    found.push({
      url: file.fileUrl!,
      label: height > 0 ? `${height}p` : 'Audio',
      quality: height > 0 ? height : undefined,
      subtitles,
    });
  }
  if (found.length === 0) throw new Error('Its instance has no file of this video to play');
  return found.slice(0, 50);
};

const server = async (id: string) => ({
  id,
  name: parseId(id).origin.replace(/^https?:\/\//, ''),
  videos: await links(id),
});

const comments = async (
  id: string,
  page: string | null | undefined,
): Promise<{ comments: VideoComment[]; next?: string | null }> => {
  await video(id);
  const { origin, uuid } = parseId(id);
  const start = Math.max(0, Number.parseInt(page ?? '0', 10) || 0);
  const answer = await fetchJson<{ total?: number; data?: Thread[] }>(
    `${origin}/api/v1/videos/${uuid}/comment-threads?${query({
      start,
      count: COMMENTS,
      sort: '-createdAt',
    })}`,
  );
  const threads = answer.data ?? [];
  const list = threads.flatMap((thread): VideoComment[] => {
    const author = thread.account?.displayName || thread.account?.name;
    const text = plainText(thread.text ?? '');
    if (thread.isDeleted || !author || !text) return [];
    // The smallest picture; the instance keeps copies of other servers' avatars itself.
    const avatar = [...(thread.account?.avatars ?? [])].sort(
      (a, b) => (a.width ?? 0) - (b.width ?? 0),
    )[0];
    return [
      {
        author,
        text,
        avatarUrl: avatar?.fileUrl ?? absolute(origin, avatar?.path),
        published: dayOf(thread.createdAt)?.iso ?? null,
        replies: thread.totalReplies ?? null,
      },
    ];
  });
  const next = start + threads.length;
  return {
    comments: list,
    next: threads.length > 0 && next < (answer.total ?? 0) ? String(next) : null,
  };
};

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'videos',
    name: 'PeerTube',
    searchPill: 'PeerTube',
    popular: ({ page }) => searchPage({ sort: '-hot', licenceOneOf: openLicences }, page),
    latest: ({ page }) => searchPage({ sort: '-publishedAt', licenceOneOf: openLicences }, page),
    filters: async () => [SORT, LICENCE, CATEGORY, LANGUAGE, DURATION],
    search: ({ query: words, page, filters: chosen }) => search(words, page, chosen),
    details: ({ id }) => details(id),
    episodes: async ({ id }) => {
      const item = await video(id);
      const published = dayOf(item.originallyPublishedAt) ?? dayOf(item.publishedAt);
      return {
        episodes: [
          {
            id,
            name: item.category?.id === 2 ? 'Film' : 'Video',
            number: 1,
            dateUpload: published?.time ?? null,
          },
        ],
      };
    },
    servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),
    videos: async ({ serverId }) => ({ videos: await links(serverId) }),
    comments: ({ id, page }) => comments(id, page),
  }),
);
