// Blender open movies: the films the Blender Studio (once the Blender Institute) made in the open
// and released under Creative Commons licences (CC BY, two of them CC BY-SA). Each plays from its
// copy on Wikimedia Commons: Commons' VP9 WebM copies at each height and the original file, with
// the subtitles volunteers wrote. The list is kept here; Commons gives the rest.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MangaPage, MangaSummary, Video, VideoSubtitle } from '@raygun/sdk';

const COMMONS = 'https://commons.wikimedia.org';
const API = `${COMMONS}/w/api.php`;
const AGENT = 'Raygun/1.0 (https://github.com/uvxszi/raygun)';
// Wikimedia asks scripts to name themselves, the files' servers included.
const MEDIA_HEADERS: Record<string, string> = { 'User-Agent': AGENT };
const API_HEADERS = { 'User-Agent': AGENT, 'Api-User-Agent': AGENT };

type Film = { id: string; title: string; year: number; file: string };

/** Best known first, which is the order Popular shows. */
const FILMS: Film[] = [
  { id: 'big-buck-bunny', title: 'Big Buck Bunny', year: 2008, file: 'Big Buck Bunny 4K.webm' },
  { id: 'sintel', title: 'Sintel', year: 2010, file: 'Sintel movie 4K.webm' },
  {
    id: 'tears-of-steel',
    title: 'Tears of Steel',
    year: 2012,
    file: 'Tears of Steel in 4k - Official Blender Foundation release.webm',
  },
  {
    id: 'elephants-dream',
    title: 'Elephants Dream',
    year: 2006,
    file: 'Elephants Dream (2006) 1080p24.webm',
  },
  {
    id: 'cosmos-laundromat',
    title: 'Cosmos Laundromat: First Cycle',
    year: 2015,
    file: 'Cosmos Laundromat - First Cycle - Official Blender Foundation release.webm',
  },
  { id: 'spring', title: 'Spring', year: 2019, file: 'Spring - Blender Open Movie.webm' },
  {
    id: 'sprite-fright',
    title: 'Sprite Fright',
    year: 2021,
    file: 'Sprite Fright - Blender Open Movie-full movie.webm',
  },
  {
    id: 'coffee-run',
    title: 'Coffee Run',
    year: 2020,
    file: 'Coffee Run - Blender Open Movie-full movie.webm',
  },
  {
    id: 'charge',
    title: 'Charge',
    year: 2022,
    file: 'Charge - Blender Open Movie-full movie.webm',
  },
  {
    id: 'wing-it',
    title: 'Wing It!',
    year: 2023,
    file: 'WING IT! - Blender Open Movie-full movie.webm',
  },
  {
    id: 'singularity',
    title: 'Singularity',
    year: 2026,
    file: 'SINGULARITY - Blender Open Movie-full movie.webm',
  },
  { id: 'hero', title: 'Hero', year: 2018, file: 'HERO - Blender Open Movie-full movie.webm' },
  {
    id: 'caminandes-llama-drama',
    title: 'Caminandes: Llama Drama',
    year: 2013,
    file: 'Caminandes- Llama Drama - Short Movie.ogv',
  },
  {
    id: 'caminandes-gran-dillama',
    title: 'Caminandes: Gran Dillama',
    year: 2013,
    file: "Caminandes - Gran Dillama - Blender Foundation's new Open Movie.webm",
  },
  {
    id: 'caminandes-llamigos',
    title: 'Caminandes 3: Llamigos',
    year: 2016,
    file: 'Caminandes 3 - Llamigos - Blender Animated Short.webm',
  },
  {
    id: 'glass-half',
    title: 'Glass Half',
    year: 2015,
    file: 'Glass Half - Blender Open Movie-full movie.webm',
  },
  {
    id: 'the-daily-dweebs',
    title: 'The Daily Dweebs',
    year: 2017,
    file: 'The Daily Dweebs - Blender Open Movie-full movie.webm',
  },
];

type Derivative = { src: string; type?: string; transcodekey?: string; height?: number };
type FileInfo = {
  url?: string;
  thumburl?: string;
  mime?: string;
  width?: number;
  height?: number;
  extmetadata?: Record<string, { value?: string | number }>;
  derivatives?: Derivative[];
  timedtext?: { src: string; srclang?: string; label?: string; kind?: string }[];
};
type QueryAnswer = {
  query?: {
    pages?: { title: string; missing?: boolean; videoinfo?: FileInfo[] }[];
    normalized?: { from: string; to: string }[];
  };
};

const filmById = (id: string) => {
  const film = FILMS.find((known) => known.id === id);
  if (!film) throw new Error('Not one of the Blender open movies');
  return film;
};

const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;|&#160;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/[^\S\n]+/g, ' ')
    .trim();

const field = (info: FileInfo, key: string) => {
  const value = info.extmetadata?.[key]?.value;
  return value === undefined ? '' : plainText(String(value));
};

/**
 * Whom the licence asks to credit, as the file's page names them: the first clause, without the
 * words before the names ("Produced by Pablo Vazquez, Beorn Leonard and Francesco Siddi. …").
 */
const creditOf = (info: FileInfo) =>
  (field(info, 'Artist').split('\n')[0] ?? '')
    .split(/;|\.\s/)[0]!
    .replace(/^(credit|author|by|produced by|directed by)\s*:?\s*/i, '')
    .trim();

const clean = (url: string | undefined) => (url ? url.replace(/[?&]utm_[^#]*$/, '') : undefined);

/** The films' Commons files, all asked for at once and kept for the session. */
let files: Promise<Map<string, FileInfo>> | null = null;
const filesOf = (): Promise<Map<string, FileInfo>> => {
  if (!files) {
    const titles = FILMS.map((film) => `File:${film.file}`).join('|');
    const params = {
      action: 'query',
      format: 'json',
      formatversion: 2,
      titles,
      prop: 'videoinfo',
      viprop: 'url|mime|size|extmetadata|derivatives|timedtext',
      viurlwidth: 640,
      viextmetadatafilter: 'ImageDescription|Artist|LicenseShortName',
      viextmetadatamultilang: 0,
    };
    const query = Object.entries(params)
      .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
      .join('&');
    files = raygun.http
      .fetch(`${API}?${query}`, { headers: API_HEADERS })
      .then(async (response) => {
        if (!response.ok) throw new Error(`Wikimedia Commons answered HTTP ${response.status}`);
        const answer = await response.json<QueryAnswer>();
        // Commons answers with its own spelling of each title ("_" for spaces and the like).
        const asked = new Map((answer.query?.normalized ?? []).map((name) => [name.to, name.from]));
        const byFile = new Map<string, FileInfo>();
        for (const page of answer.query?.pages ?? []) {
          const info = page.videoinfo?.[0];
          if (page.missing || !info) continue;
          byFile.set((asked.get(page.title) ?? page.title).replace(/^File:/, ''), info);
        }
        return byFile;
      });
    files.catch(() => {
      files = null;
    });
  }
  return files;
};

const infoOf = async (film: Film) => {
  const info = (await filesOf()).get(film.file);
  if (!info) throw new Error(`${film.title} is no longer on Wikimedia Commons`);
  return info;
};

const listing = async (films: Film[]): Promise<MangaPage> => {
  const known = await filesOf().catch(() => new Map<string, FileInfo>());
  const items: MangaSummary[] = films.map((film) => ({
    id: film.id,
    title: film.title,
    coverUrl: clean(known.get(film.file)?.thumburl) ?? null,
  }));
  return { items, hasNextPage: false };
};

const nominalHeight = (key: string | undefined) => Number.parseInt(key ?? '', 10) || undefined;

/** Commons' VP9 WebM copies, best first, then the original when it is WebM. */
const videosOf = (info: FileInfo): Video[] => {
  const subtitles: VideoSubtitle[] = (info.timedtext ?? [])
    .filter((track) => track.src && track.kind !== 'metadata')
    .map((track) => ({ url: track.src, lang: track.srclang, label: track.label }));
  const videos: Video[] = (info.derivatives ?? [])
    .filter((copy) => /^video\/webm/.test(copy.type ?? '') && /vp9|vp8/i.test(copy.type ?? ''))
    .map((copy) => {
      const height = nominalHeight(copy.transcodekey) ?? copy.height;
      return {
        url: copy.src,
        label: height ? `${height}p · VP9` : 'VP9',
        quality: height,
        headers: MEDIA_HEADERS,
        subtitles,
      };
    });
  const original = clean(info.url);
  if (original && info.mime === 'video/webm') {
    videos.push({
      url: original,
      label: info.height ? `Original · ${info.width}×${info.height}` : 'Original',
      quality: info.height || undefined,
      headers: MEDIA_HEADERS,
      subtitles,
    });
  }
  return videos.sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
};

const server = async (id: string) => {
  const film = filmById(id);
  const videos = videosOf(await infoOf(film));
  if (videos.length === 0)
    throw new Error(`Commons has no copy of ${film.title} that Raygun can play`);
  return { id, name: 'Wikimedia Commons', videos };
};

/** Combining accents, left over once letters are taken apart (é → e + ´). */
const ACCENTS = new RegExp(`[${String.fromCharCode(0x300)}-${String.fromCharCode(0x36f)}]`, 'g');

const words = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(ACCENTS, '')
    .split(/[^a-z0-9]+/)
    .filter(Boolean);

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'movies',
    name: 'Blender open movies',

    popular: async ({ page }) => (page > 1 ? { items: [], hasNextPage: false } : listing(FILMS)),

    latest: async ({ page }) =>
      page > 1
        ? { items: [], hasNextPage: false }
        : listing([...FILMS].sort((a, b) => b.year - a.year)),

    async search({ query, page }) {
      if (page > 1) return { items: [], hasNextPage: false };
      const wanted = words(query);
      return listing(
        FILMS.filter((film) => {
          const offered = [...words(film.title), 'blender', String(film.year)];
          return wanted.every((word) => offered.some((have) => have.startsWith(word)));
        }),
      );
    },

    async details({ id }) {
      const film = filmById(id);
      const info = await infoOf(film);
      const about = field(info, 'ImageDescription');
      const licence = field(info, 'LicenseShortName');
      const credit = creditOf(info);
      return {
        id,
        title: film.title,
        coverUrl: clean(info.thumburl) ?? null,
        description:
          [
            about,
            [licence && `Licence: ${licence}`, credit && `By ${credit}`]
              .filter(Boolean)
              .join(' · '),
          ]
            .filter(Boolean)
            .join('\n\n') || null,
        // Tears of Steel is the one live-action film among them.
        genres:
          id === 'tears-of-steel' ? ['Science fiction', 'Short film'] : ['Animation', 'Short film'],
        directors: [],
        type: 'movie',
        year: film.year,
        status: 'completed',
        url: `${COMMONS}/wiki/File:${encodeURIComponent(film.file.replace(/ /g, '_'))}`,
      };
    },

    episodes: async ({ id }) => ({ episodes: [{ id: filmById(id).id, name: 'Film' }] }),

    servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),

    videos: async ({ serverId }) => ({ videos: (await server(serverId)).videos }),
  }),
);
