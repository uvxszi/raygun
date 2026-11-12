// The UCSB Cylinder Audio Archive (cylinders.library.ucsb.edu): the UC Santa Barbara Library's
// transfers of wax and celluloid cylinder records, the 1890s to the 1920s. US sound recordings
// released before 1923 entered the public domain in 2022, and later ones 100 years after the year
// of their release (17 U.S.C. § 1401), so only cylinders released more than 100 years ago are
// offered, and only issued ones: home and field recordings never released stay protected until
// 2067. The library makes its transfers of cylinders from before 1923 free for any use and
// licenses the rest under CC BY-NC 2.5 (credit: "University of California, Santa Barbara
// Library"): https://cylinders.library.ucsb.edu/licensing.php.
//
// The archive has no API, so its own pages are read: a search (25 results a page, with their
// release years), then each cylinder's page for its number and MP3, read once and kept. Charts are
// subject searches. Chart rows leave out titles with racial slurs, which some records of the
// time carry; a search still finds them.
import { defineProvider, fetchText, query, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const SITE = 'https://cylinders.library.ucsb.edu';
/** Cylinder pages read at once: the archive is a library's own server. */
const AT_ONCE = 5;
const SEARCH_MOST = 10;
const CHART_MOST = 15;
const LIST_TTL_MS = 60 * 60_000;
const KEPT_MOST = 800;

/** The latest release year offered: released more than 100 years ago (1925 in 2026). */
const cutoff = () => new Date().getUTCFullYear() - 101;

/** Slurs in some titles of the time; chart rows leave those records out. */
const SLUR = /\b(?:nigg\w*|coons?|dark(?:e?y|ies)|pickaninn\w*|chinks?|japs?|kikes?|wops?)\b/i;

/** A cylinder as kept: its number, MP3, title, performers, issue and release year. */
type Cylinder = {
  number: string;
  /** Its MP3, on the archive's storage. */
  url: string;
  title: string;
  performers: string | null;
  issue: string | null;
  year: number;
};
/** A search result: the record's id in the library's catalogue and its release year. */
type Result = { mms: string; year: number | null };

const decode = (text: string) =>
  text
    .replace(/<[^>]*>/g, ' ')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&nbsp;/g, ' ')
    .replace(/&mdash;/g, '—')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();

/** The latest year a text names: "between 1904 and 1909" is 1909; `null` without one. */
const latestYear = (text: string): number | null => {
  const years = [...text.matchAll(/\b(1[89]\d\d)\b/g)].map((match) => Number(match[1]));
  return years.length > 0 ? Math.max(...years) : null;
};

/**
 * Catalogue punctuation off: "Alexander's ragtime band /" → "Alexander's ragtime band",
 * "[Steve] Porter" → "Steve Porter", 'The "knocker" :. (recitation)' → 'The "knocker": (…)'.
 */
const tidy = (text: string) =>
  text
    .replace(/[[\]]/g, '')
    .replace(/\s+([:;,.])/g, '$1')
    .replace(/([:;,])\./g, '$1')
    .replace(/\s*[/:;,.]\s*$/, '')
    .replace(/\s+/g, ' ')
    .trim();

/**
 * Recordings never issued (home and field recordings, test takes): the public-domain rule for
 * old recordings counts from release, so only issued records are offered.
 */
const UNISSUED =
  /home recording|field recording|unissued|unpublished|private recording|test record/i;

const capitalized = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** A page's rows as label and entries: "Performers:" → ["Billy Murray."]. */
const rowsOf = (html: string): Map<string, string[]> => {
  const rows = new Map<string, string[]>();
  // Each row is a `pure-g` block: its label, then one entry or several.
  for (const block of html.split(/<div class="pure-g\b[^"]*">/).slice(1)) {
    const label = decode(/data-label">([^<]*)</.exec(block)?.[1] ?? '')
      .replace(/:$/, '')
      .toLowerCase();
    const entries = [...block.matchAll(/<div class="data-entry">([\s\S]*?)<\/div>/g)]
      .map((entry) => decode(entry[1] ?? ''))
      .filter(Boolean);
    if (label && !rows.has(label)) rows.set(label, entries);
  }
  return rows;
};

/** The results of a search page, each with its release year when the page gives one. */
const resultsOf = (html: string): Result[] => {
  const seen = new Set<string>();
  return [...html.matchAll(/<div class="result-row">([\s\S]*?)<hr \/>/g)].flatMap((row) => {
    const mms = /detail\.php\?query_type=mms_id&(?:amp;)?query=(\d+)/.exec(row[1] ?? '')?.[1];
    if (!mms || seen.has(mms)) return [];
    seen.add(mms);
    const year =
      rowsOf(row[1] ?? '')
        .get('release year')
        ?.join(' ') ?? '';
    return [{ mms, year: latestYear(year) }];
  });
};

/**
 * A cylinder's page read, or null when it has nothing to play, was released too recently or
 * wasn't issued: an issued cylinder has an issue number or is catalogued as commercial.
 */
const cylinderOf = (html: string): Cylinder | null => {
  const title = tidy(decode(/<h1[^>]*>([\s\S]*?)<\/h1>/.exec(html)?.[1] ?? ''));
  const audio = /<audio[^>]*\ssrc="(https:\/\/cylinders\.s3[^"]+\.mp3)"/.exec(html)?.[1];
  const number = /cusb-cyl(\d+)[a-z]?\.mp3/i.exec(audio ?? '')?.[1];
  const rows = rowsOf(html);
  const year = latestYear((rows.get('release year') ?? []).join(' '));
  if (!title || !number || year === null || year > cutoff()) return null;
  const issue = tidy((rows.get('issue number') ?? [])[0] ?? '').replace(/:\s*/, ' ');
  const catalogued = [title, ...(rows.get('notes') ?? []), ...(rows.get('genre') ?? [])].join('\n');
  if (UNISSUED.test(catalogued) || (!issue && !/commercial/i.test(catalogued))) return null;
  const performers = tidy((rows.get('performers') ?? []).join('; '));
  return {
    number,
    url: audio!,
    title: capitalized(title),
    performers: /not identified|^unknown/i.test(performers) || !performers ? null : performers,
    issue: issue || null,
    year,
  };
};

// ---- Cylinder pages, kept ----

/** Bump when what's kept changes, so older pages are read again. */
const KEPT = 'cylinders.v1';
let kept: Map<string, Cylinder | null> | null = null;
const keptCylinders = () => {
  if (!kept) {
    let saved: unknown = null;
    try {
      saved = raygun.storage.get(KEPT);
    } catch {
      // Without a store, pages are kept this run.
    }
    kept = new Map(Array.isArray(saved) ? (saved as [string, Cylinder | null][]) : []);
  }
  return kept;
};
const keep = (mms: string, cylinder: Cylinder | null) => {
  const cylinders = keptCylinders();
  cylinders.delete(mms);
  cylinders.set(mms, cylinder);
  while (cylinders.size > KEPT_MOST) cylinders.delete(cylinders.keys().next().value!);
  try {
    raygun.storage.set(KEPT, [...cylinders.entries()]);
  } catch {
    // A full store keeps them for this run.
  }
};

/** A cylinder by its catalogue id: kept, or read from its page. */
const cylinder = async (mms: string): Promise<Cylinder | null> => {
  const cylinders = keptCylinders();
  if (cylinders.has(mms)) return cylinders.get(mms) ?? null;
  const html = await fetchText(`${SITE}/detail.php?${query({ query_type: 'mms_id', query: mms })}`);
  const found = cylinderOf(html);
  keep(mms, found);
  return found;
};

const trackOf = (mms: string, found: Cylinder): Track => ({
  id: mms,
  title: found.title,
  artist: found.performers,
  album: found.issue,
  source: {
    kind: 'url',
    url: found.url,
    headers: {},
  },
});

/** The cylinders of a search's results, as songs, a few pages at a time. */
const tracksOf = async (results: Result[], most: number): Promise<Track[]> => {
  const wanted = results.filter((result) => result.year === null || result.year <= cutoff());
  const tracks: Track[] = [];
  for (let at = 0; at < wanted.length && tracks.length < most; at += AT_ONCE) {
    const batch = wanted.slice(at, at + AT_ONCE);
    const read = await Promise.allSettled(batch.map((result) => cylinder(result.mms)));
    read.forEach((answer, index) => {
      if (answer.status === 'fulfilled' && answer.value) {
        tracks.push(trackOf(batch[index]!.mms, answer.value));
      } else if (answer.status === 'rejected') {
        raygun.log.warn('cylinder skipped:', String(answer.reason));
      }
    });
  }
  return tracks.slice(0, most);
};

const searchPage = async (type: 'keyword' | 'subject', words: string): Promise<Result[]> =>
  resultsOf(
    await fetchText(`${SITE}/search.php?${query({ nq: 1, query_type: type, query: words })}`),
  );

// ---- Charts: subject searches ----

const CHARTS: (Chart & { subject: string })[] = [
  {
    id: 'ragtime',
    name: 'Ragtime on cylinders',
    tag: null,
    group: 'genre',
    subject: 'Ragtime music',
  },
  { id: 'vaudeville', name: 'Vaudeville', tag: null, group: 'genre', subject: 'Vaudeville' },
  { id: 'opera', name: 'Opera on cylinders', tag: 'Classical', group: 'genre', subject: 'Operas' },
  { id: 'marches', name: 'Marches', tag: null, group: 'genre', subject: 'Marches' },
  { id: 'banjo', name: 'Banjo', tag: 'Folk', group: 'genre', subject: 'Banjo music' },
  { id: 'hawaiian', name: 'Hawaiian music', tag: null, group: 'genre', subject: 'Hawaiian' },
  { id: 'yodels', name: 'Yodels', tag: null, group: 'genre', subject: 'Yodels' },
  { id: 'hymns', name: 'Hymns', tag: null, group: 'genre', subject: 'Hymns' },
  { id: 'christmas', name: 'Christmas', tag: null, group: 'genre', subject: 'Christmas music' },
];

const lists = new Map<string, { at: number; tracks: Promise<Track[]> }>();

const chartTracks = (id: string): Promise<Track[]> => {
  const chart = CHARTS.find((each) => each.id === id);
  if (!chart) return Promise.reject(new Error('The Cylinder Audio Archive has no such chart'));
  const listed = lists.get(id);
  if (listed && Date.now() - listed.at < LIST_TTL_MS) return listed.tracks;
  const current = {
    at: Date.now(),
    tracks: searchPage('subject', chart.subject)
      .then((results) => tracksOf(results, CHART_MOST + 5))
      .then((tracks) => tracks.filter((track) => !SLUR.test(track.title)).slice(0, CHART_MOST)),
  };
  current.tracks.catch(() => {
    if (lists.get(id) === current) lists.delete(id);
  });
  lists.set(id, current);
  return current.tracks;
};

registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'cylinders',
    name: 'UCSB Cylinder Audio Archive',

    async search({ query: typed, limit }) {
      const words = typed.replace(/\s+/g, ' ').trim();
      if (!words) return { tracks: [] };
      const results = await searchPage('keyword', words);
      return { tracks: await tracksOf(results, Math.max(1, Math.min(limit, SEARCH_MOST))) };
    },
  }),
);

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'UCSB Cylinder Audio Archive',

    charts: async () => ({
      charts: CHARTS.map(({ id, name, tag, group }) => ({ id, name, tag, group })),
    }),

    async chart({ id, limit }) {
      return { tracks: (await chartTracks(id)).slice(0, Math.max(1, limit)) };
    },

    async tagSongs({ tag, aliases, kind, limit }) {
      if (kind !== 'genre') return { tracks: [] };
      const names = [tag, ...aliases].map((name) => name.toLowerCase());
      const id = names.includes('ragtime') ? 'ragtime' : names.includes('opera') ? 'opera' : null;
      if (!id) return { tracks: [] };
      return { tracks: (await chartTracks(id)).slice(0, Math.max(1, limit)) };
    },
  }),
);
