// ESA/Hubble (esahubble.org) and ESA/Webb (esawebb.org): the European Space Agency's Hubble and
// Webb videos, one provider each. Both sites release their images and videos under Creative
// Commons Attribution 4.0 (esahubble.org/copyright, esawebb.org/copyright), credited as each
// video's page says. A video whose page names another licence (a CC BY-NC soundtrack, a copyright,
// a permission to ask for) is refused when it's opened, and lists leave it out from then on. The
// sites' terms keep music out of the licence (it's for their own videos); the videos play
// unchanged from their servers, as Wikimedia Commons hosts them, and the details say so.
//
// The sites have no API, so pages are read as they are: the main list (`/videos/page/<n>/`, the
// sites' own order, Hubblecasts first), every video newest first (`/videos/viewall/` sorted by
// release date), search and categories (`?search=`, `/videos/archive/category/<slug>/`), and a
// video's page (`/videos/<id>/`) for its text, credit and files. Pages are kept in memory, and a
// video's page is read once for its details, episodes and files.
import { defineProvider, fetchText, registerProvider, selectValue } from '@raygun/sdk';
import type { MangaPage, SelectFilter, Video, VideoSubtitle } from '@raygun/sdk';

import { hasPageAfter, tableRows, thumbnails, videoPage } from './pages';
import type { Listed, VideoFile, VideoPage } from './pages';

const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';

type Site = {
  id: string;
  name: string;
  base: string;
  cdn: string;
  categories: SelectFilter;
};

const categories = (options: [string, string][]): SelectFilter => ({
  kind: 'select',
  key: 'category',
  label: 'Category',
  options: [
    { value: 'all', label: 'All videos' },
    ...options.map(([value, label]) => ({ value, label })),
  ],
  default: 'all',
});

const HUBBLE: Site = {
  id: 'hubble',
  name: 'ESA/Hubble',
  base: 'https://esahubble.org',
  cdn: 'https://cdn.esahubble.org',
  categories: categories([
    ['hubblecast', 'Hubblecast'],
    ['spacesparks', 'Space Sparks'],
    ['galaxies', 'Galaxies'],
    ['nebulae', 'Nebulae'],
    ['stars', 'Stars'],
    ['clusters', 'Star Clusters'],
    ['solarsystem', 'Solar System'],
    ['extrasolar', 'Exoplanets'],
    ['blackholes', 'Quasars and Black Holes'],
    ['cosmology', 'Cosmology'],
    ['spacecraft', 'Spacecraft'],
    ['jwst', 'James Webb Space Telescope'],
    ['3d', '3D Animations'],
    ['fulldome', 'Fulldome'],
    ['hd', 'HD'],
    ['hubble', 'Hubble Images'],
    ['dvd', 'Hubble 15 Years DVD'],
    ['eyesontheskies', 'Eyes on the Skies DVD'],
    ['misc', 'Miscellaneous'],
  ]),
};

const WEBB: Site = {
  id: 'webb',
  name: 'ESA/Webb',
  base: 'https://esawebb.org',
  cdn: 'https://cdn.esawebb.org',
  categories: categories([
    ['spacesparks', 'Space Sparks'],
    ['pictureofthemonth', 'Picture of the Month'],
    ['zooms', 'Zooms'],
    ['pans', 'Pans'],
    ['transitions', 'Transitions'],
    ['animations', 'Animations'],
    ['sonification', 'Sonification'],
    ['galaxies', 'Galaxies'],
    ['nebulae', 'Nebulae'],
    ['stars', 'Stars'],
    ['exoplanets', 'Exoplanets'],
    ['solarsystem', 'Solar System'],
    ['quasarsblackholes', 'Quasars and Black Holes'],
    ['instruments', 'Instruments'],
    ['miri', 'MIRI'],
    ['nircam', 'NIRCam'],
    ['niriss', 'NIRISS'],
    ['nirspec', 'NIRSpec'],
    ['launch', 'Launch'],
    ['anniversary', 'Anniversary'],
  ]),
};

/**
 * The files Raygun plays, by the folder the site keeps them in, with their heights (the sites'
 * "Video formats" page). H.265, broadcast, Flash and MPEG-1 copies are left out; QuickTime copies
 * only count for old videos with nothing else.
 */
const FORMATS: { folder: string; height: number; label: string; quickTime?: boolean }[] = [
  { folder: 'ultra_hd', height: 2160, label: '4K' },
  { folder: 'qhd_1440p25_screen', height: 1440, label: '1440p' },
  { folder: 'hd_1080p25_screen', height: 1080, label: '1080p' },
  { folder: 'dome_preview', height: 1024, label: 'Fulldome preview' },
  { folder: 'hd_and_apple', height: 720, label: '720p' },
  { folder: 'large_qt', height: 405, label: '405p', quickTime: true },
  { folder: 'medium_podcast', height: 360, label: '360p' },
  { folder: 'small_qt', height: 180, label: '180p', quickTime: true },
];

/** Licence words on a video's page that aren't CC BY's: those videos aren't offered. */
const OTHER_LICENCE =
  /©|\(c\)\s*(?:19|20)\d\d|all rights reserved|\bcopyright(?:ed)?\b|\bpermission\b|non-?commercial|no-?derivatives?|\bCC[\s-]*BY[\s-]*(?:NC|ND)\b|creativecommons\.org\/licenses\/by-n[cd]|not (?:be )?(?:used|reproduced|released under)/i;

const LANGUAGES: Record<string, string> = {
  ar: 'Arabic',
  bg: 'Bulgarian',
  bs: 'Bosnian',
  ca: 'Catalan',
  cs: 'Czech',
  da: 'Danish',
  de: 'German',
  el: 'Greek',
  en: 'English',
  es: 'Spanish',
  fi: 'Finnish',
  fr: 'French',
  he: 'Hebrew',
  hi: 'Hindi',
  hr: 'Croatian',
  hu: 'Hungarian',
  is: 'Icelandic',
  it: 'Italian',
  iw: 'Hebrew',
  ja: 'Japanese',
  ko: 'Korean',
  lv: 'Latvian',
  nl: 'Dutch',
  no: 'Norwegian',
  pl: 'Polish',
  pt: 'Portuguese',
  ro: 'Romanian',
  ru: 'Russian',
  sk: 'Slovak',
  sl: 'Slovenian',
  sr: 'Serbian',
  sv: 'Swedish',
  sw: 'Swahili',
  th: 'Thai',
  tr: 'Turkish',
  uk: 'Ukrainian',
  vi: 'Vietnamese',
  zh: 'Chinese',
};

/** Puts `value` last in a map that keeps its `most` latest entries. */
const remember = <V>(map: Map<string, V>, key: string, value: V, most: number) => {
  map.delete(key);
  map.set(key, value);
  if (map.size > most) map.delete(map.keys().next().value!);
};

/** List pages shown lately, by address. */
const lists = new Map<string, MangaPage>();
/** Video pages read lately, by site and id. */
const videos = new Map<string, VideoPage>();

/**
 * Videos a site's page showed under another licence, kept across restarts (`refused.<site>`, the
 * latest 500), so lists leave them out once one was opened.
 */
const refusals = new Map<string, Set<string>>();
const refusedOn = (site: Site): Set<string> => {
  let refused = refusals.get(site.id);
  if (!refused) {
    let saved: string[] | null = null;
    try {
      saved = raygun.storage.get<string[]>(`refused.${site.id}`);
    } catch {
      // Without a store, refusals last this run.
    }
    refused = new Set(Array.isArray(saved) ? saved : []);
    refusals.set(site.id, refused);
  }
  return refused;
};
const refuse = (site: Site, id: string) => {
  const refused = refusedOn(site);
  if (refused.has(id)) return;
  refused.add(id);
  try {
    raygun.storage.set(`refused.${site.id}`, [...refused].slice(-500));
  } catch {
    // A full store keeps the refusal for this run only.
  }
};

const fetchPage = (url: string) =>
  fetchText(url, { headers: { 'User-Agent': AGENT, Accept: 'text/html' } });

const coverOf = (site: Site, video: Listed) =>
  video.coverUrl ?? `${site.cdn}/archives/videos/videoframe/${encodeURIComponent(video.id)}.jpg`;

/** One page of a list, read with `read` (`thumbnails` or `tableRows`). */
const listPage = async (
  site: Site,
  url: string,
  page: number,
  read: (html: string) => Listed[],
): Promise<MangaPage> => {
  let listed = lists.get(url);
  if (!listed) {
    const html = await fetchPage(url);
    const found = read(html);
    listed = {
      items: found.map((video) => ({
        id: video.id,
        title: video.title,
        coverUrl: coverOf(site, video),
      })),
      hasNextPage: found.length > 0 && hasPageAfter(html, page),
    };
    remember(lists, url, listed, 40);
  }
  const refused = refusedOn(site);
  return { ...listed, items: listed.items.filter((item) => !refused.has(item.id)) };
};

/** A video's page, read once; refused when it names a licence other than CC BY. */
const videoOf = async (site: Site, id: string): Promise<VideoPage> => {
  if (!/^[A-Za-z0-9_-]+$/.test(id)) throw new Error(`${site.name} has no video ${id}`);
  const key = `${site.id}:${id}`;
  let video = videos.get(key);
  if (!video) {
    const url = `${site.base}/videos/${id}/`;
    const response = await raygun.http.fetch(url, {
      headers: { 'User-Agent': AGENT, Accept: 'text/html' },
    });
    if (response.status === 404) throw new Error(`This video is no longer on ${site.name}'s site`);
    if (!response.ok) throw new Error(`${site.name}'s site answered HTTP ${response.status}`);
    const read = videoPage(await response.text(), site.base, id);
    if (!read) throw new Error(`${site.name}'s page for this video couldn't be read`);
    video = read;
  }
  remember(videos, key, video, 60);
  if (OTHER_LICENCE.test([video.description, ...video.credit].join('\n'))) {
    refuse(site, id);
    throw new Error(
      `${site.name} names a licence other than CC BY for part of this video, so Raygun doesn't offer it`,
    );
  }
  return video;
};

/** The files Raygun plays, best first. */
const playable = (video: VideoPage): (VideoFile & { height: number; label: string })[] => {
  const known = video.files.flatMap((file) => {
    const format = FORMATS.find((candidate) => candidate.folder === file.folder);
    if (!format || !/\.(?:mp4|m4v|mov)$/i.test(file.url)) return [];
    return [{ ...file, height: format.height, label: format.label, quickTime: format.quickTime }];
  });
  const modern = known.filter((file) => !file.quickTime);
  return (modern.length > 0 ? modern : known).sort((a, b) => b.height - a.height);
};

/** "01 m 45 s", "1 h 21 m 09 s", "30 s" as a clock: "1:45". */
const clockOf = (duration: string | undefined) => {
  const part = (unit: string) =>
    Number(new RegExp(`(\\d+)\\s*${unit}\\b`).exec(duration ?? '')?.[1] ?? 0);
  const [hours, minutes, seconds] = [part('h'), part('m'), part('s')];
  if (hours + minutes + seconds === 0) return '';
  const rest = String(seconds).padStart(2, '0');
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
};

const MONTHS = [
  'january',
  'february',
  'march',
  'april',
  'may',
  'june',
  'july',
  'august',
  'september',
  'october',
  'november',
  'december',
];

/** "10 December 2020, 17:00" in Unix milliseconds (the sites' own time, read as UTC). */
const releasedAt = (date: string | undefined): number | null => {
  const parts = /(\d{1,2})\s+([A-Za-z]+)\s+(\d{4})(?:,\s*(\d{1,2}):(\d{2}))?/.exec(date ?? '');
  if (!parts) return null;
  const month = MONTHS.indexOf((parts[2] ?? '').toLowerCase());
  if (month < 0) return null;
  return Date.UTC(
    Number(parts[3]),
    month,
    Number(parts[1]),
    Number(parts[4] ?? 0),
    Number(parts[5] ?? 0),
  );
};

const descriptionOf = (site: Site, video: VideoPage) => {
  const facts = [
    video.facts['Release date'] ? `Released ${video.facts['Release date']}` : '',
    clockOf(video.facts.Duration),
    video.facts.Name ? `Object: ${video.facts.Name}` : '',
  ].filter(Boolean);
  const music = video.credit.some((line) => /^music\s*:/i.test(line));
  return [
    video.description,
    facts.join(' · '),
    `Credit: ${video.credit.join('; ') || site.name}`,
    `Licence: CC BY 4.0 (${site.base}/copyright/).${
      music
        ? ` The music isn't part of that licence: ${site.name} lets it be used only in its own videos, which play here as it published them.`
        : ''
    }`,
  ]
    .filter(Boolean)
    .join('\n\n');
};

const videosOf = async (site: Site, id: string): Promise<Video[]> => {
  const video = await videoOf(site, id);
  const subtitles: VideoSubtitle[] = video.subtitles.map(({ url, lang }) => ({
    url,
    lang,
    label: LANGUAGES[lang.toLowerCase()] ?? lang,
  }));
  const files = playable(video);
  if (files.length === 0) throw new Error(`${site.name} has no file of this video Raygun can play`);
  return files.map((file) => ({
    url: file.url,
    label: file.label,
    quality: file.height,
    subtitles,
  }));
};

const providerOf = (site: Site) =>
  defineProvider({
    kind: 'video.source',
    id: site.id,
    name: site.name,

    // The site's own order: its Hubblecasts and highlights first.
    popular: ({ page }) =>
      listPage(site, `${site.base}/videos/page/${Math.max(1, page)}/`, page, thumbnails),

    latest: ({ page }) =>
      listPage(
        site,
        `${site.base}/videos/viewall/page/${Math.max(1, page)}/?sort=-release_date`,
        page,
        tableRows,
      ),

    search({ query, page, filters }) {
      const category = selectValue(filters, site.categories);
      const words = query.trim();
      const path = category === 'all' ? '/videos' : `/videos/archive/category/${category}`;
      const search = words ? `?search=${encodeURIComponent(words)}` : '';
      return listPage(
        site,
        `${site.base}${path}/page/${Math.max(1, page)}/${search}`,
        page,
        thumbnails,
      );
    },

    filters: async () => [site.categories],

    async details({ id }) {
      const video = await videoOf(site, id);
      const released = releasedAt(video.facts['Release date']);
      return {
        id,
        title: video.title,
        coverUrl: `${site.cdn}/archives/videos/videoframe/${encodeURIComponent(id)}.jpg`,
        description: descriptionOf(site, video),
        genres: video.categories.slice(0, 8),
        type: 'other',
        year: released === null ? null : new Date(released).getUTCFullYear(),
        status: 'completed',
        url: `${site.base}/videos/${encodeURIComponent(id)}/`,
      };
    },

    async episodes({ id }) {
      const video = await videoOf(site, id);
      if (playable(video).length === 0) {
        throw new Error(`${site.name} has no file of this video Raygun can play`);
      }
      return {
        episodes: [{ id, name: 'Video', dateUpload: releasedAt(video.facts['Release date']) }],
      };
    },

    // One server, resolved at once: the site serves every file itself.
    servers: async ({ episodeId }) => ({
      servers: [{ id: episodeId, name: site.name, videos: await videosOf(site, episodeId) }],
    }),

    videos: async ({ serverId }) => ({ videos: await videosOf(site, serverId) }),
  });

registerProvider(providerOf(HUBBLE));
registerProvider(providerOf(WEBB));
