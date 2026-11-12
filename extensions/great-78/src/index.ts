// The Great 78 Project: 78 rpm records transferred by George Blood L.P. for the Internet Archive
// and its partner libraries, limited to sides recorded more than 101 years ago. US sound
// recordings published before 1923 entered the public domain in 2022, and later ones 100 years
// after the year of their publication (17 U.S.C. § 1401); a side's year is the year it was
// recorded, so a year's margin covers records issued the year after. Later sides are left out:
// their rights are what record labels and the Archive disputed. So are discs never issued (test
// pressings, home and field recordings), which that rule doesn't reach.
//
// Songs, charts (most played, by decade and by genre) and genre rows come from the Archive's
// advanced search; each side's record is then read for its MP3 (the restored transfer when there
// is one) and checked again. Chart and genre rows leave out titles with racial slurs, which some
// records of the time carry; a search still finds them.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const API = 'https://archive.org';
const COLLECTION = 'georgeblood';
/** Charts and genre rows: the most a list holds, and how long it's kept. */
const LIST_MOST = 30;
const LIST_TTL_MS = 30 * 60_000;
/** Records read at once. */
const AT_ONCE = 10;

/** The latest year whose sides are offered: recorded more than 101 years ago (1924 in 2026). */
const cutoff = () => new Date().getUTCFullYear() - 102;

const allowed = () => `collection:${COLLECTION} AND mediatype:audio AND year:[1880 TO ${cutoff()}]`;

/** Records that were never issued: the public-domain rule counts from release. */
const UNISSUED =
  /\b(?:unissued|unreleased|unpublished|test pressing|home recording|field recording|private recording)\b/i;

/** Slurs in some titles of the time; rows shown unasked leave those records out. */
const SLUR = /\b(?:nigg\w*|coons?|dark(?:e?y|ies)|pickaninn\w*|chinks?|japs?|kikes?|wops?)\b/i;

type SearchDoc = { identifier: string };
type SearchAnswer = { response?: { numFound?: number; docs?: SearchDoc[] } };

type ItemFile = { name: string; format?: string; length?: string; title?: string };
type ItemRecord = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    creator?: string | string[];
    description?: string | string[];
    date?: string | string[];
    year?: string | string[];
    publisher?: string | string[];
    'publisher-catalog-number'?: string | string[];
    collection?: string | string[];
  };
};

// Lucene syntax characters would turn what was typed into a different search.
const escapeQuery = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const first = (value: string | string[] | undefined): string =>
  ((Array.isArray(value) ? value[0] : value) ?? '').trim();

const decode = (text: string) =>
  text
    .replace(/<br\s*\/?>|<\/?p>/gi, '\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .trim();

/** "ORY'S CREOLE TROMBONE" → "Ory's Creole Trombone"; text with lower case stays as it is. */
const unshouted = (text: string) =>
  /\p{Ll}/u.test(text)
    ? text
    : text
        .toLowerCase()
        .replace(
          /(^|[\s(["/-])(\p{L})/gu,
          (_match, before: string, letter: string) => `${before}${letter.toUpperCase()}`,
        );

/** "KID ORY'S SUNSHINE ORCHESTRA; Papa Mutt Carey; …": the first three, each as it's written. */
const peopleOf = (names: string) =>
  names
    .split(/\s*;\s*/)
    .filter(Boolean)
    .slice(0, 3)
    .map(unshouted)
    .join('; ');

/** The performers a record's description names ("Performer: Billy Murray<br />Writer: …"). */
const performerOf = (description: string): string | null => {
  const line = /Performers?:\s*([^\n]+)/i.exec(decode(description))?.[1]?.trim();
  return line ? line.replace(/[.;,]\s*$/, '') : null;
};

/**
 * The latest year any of the texts names: a set dated "1907 - 1936" whose notes say "Okemah,
 * Okla., 1940" is 1940. `null` without a year.
 */
const latestYear = (...texts: string[]): number | null => {
  const years = texts.flatMap((text) =>
    [...text.matchAll(/\b(1[89]\d\d|20\d\d)\b/g)].map((match) => Number(match[1])),
  );
  return years.length > 0 ? Math.max(...years) : null;
};

// `length` is seconds ("245.32") or a clock ("4:05").
const lengthMs = (length: string | undefined): number | null => {
  if (!length) return null;
  const seconds = length.split(':').reduce((total, part) => total * 60 + Number(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

/** A file's name without the "-restored" a restored transfer adds. */
const unrestored = (name: string) => name.replace(/[-_ ]*\(?restored\)?(?=\.mp3$)/i, '');

/**
 * The MP3s to play, in the record's order: one per recording (a set of discs has several), the
 * restored transfer when there is one.
 */
const mp3sOf = (files: ItemFile[]): ItemFile[] => {
  const byRecording = new Map<string, ItemFile>();
  for (const file of files) {
    if (!/mp3/i.test(file.format ?? '') || !/\.mp3$/i.test(file.name)) continue;
    const key = unrestored(file.name);
    const kept = byRecording.get(key);
    if (!kept || (/restored/i.test(file.name) && !/restored/i.test(kept.name))) {
      byRecording.set(key, file);
    }
  }
  return [...byRecording.entries()]
    .sort(([one], [other]) => one.localeCompare(other, 'en', { numeric: true }))
    .map(([, file]) => file);
};

const records = new Map<string, Promise<ItemRecord>>();
const recordOf = (identifier: string): Promise<ItemRecord> => {
  let record = records.get(identifier);
  if (!record) {
    record = fetchJson<ItemRecord>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    record.catch(() => records.delete(identifier));
    records.set(identifier, record);
    if (records.size > 300) records.delete(records.keys().next().value!);
  }
  return record;
};

/** A file's own title, without the "(restored)" its restored transfer carries. */
const fileTitle = (file: ItemFile) =>
  decode(file.title ?? '')
    .replace(/\s*\(restored\)\s*$/i, '')
    .trim();

/**
 * A record's recordings as songs: none when it doesn't hold up (not the project's, recorded too
 * recently, nothing to play), and at most 12 from a set.
 */
const recordingsOf = async (identifier: string): Promise<Track[]> => {
  const record = await recordOf(identifier);
  const metadata = record.metadata ?? {};
  const collections = [metadata.collection ?? []].flat();
  // A record holds up when every year it names is old enough: its date (a range for a set),
  // the index's year and the years its notes give.
  const year = latestYear(
    [metadata.year ?? []].flat().join(' '),
    [metadata.date ?? []].flat().join(' '),
    // Matrix and catalogue numbers aren't years.
    decode([metadata.description ?? []].flat().join('\n')).replace(
      /\b(?:matrix|catalog(?:ue)?|take)\s+numbers?:[^\n]*/gi,
      '',
    ),
  );
  if (!collections.includes(COLLECTION) || year === null || year > cutoff()) return [];
  // The rule counts from release: discs never issued (test pressings, home and field
  // recordings) stay protected until 2067.
  const notes = decode([metadata.title ?? [], metadata.description ?? []].flat().join('\n'));
  if (UNISSUED.test(notes)) return [];
  const title = unshouted(decode(first(metadata.title)));
  const files = mp3sOf(record.files ?? []).slice(0, 12);
  if (files.length === 0 || !title) return [];
  const label = decode(first(metadata.publisher));
  const number = first(metadata['publisher-catalog-number']);
  const album = label ? `${unshouted(label)}${number ? ` ${number}` : ''}` : null;
  const performer = performerOf(first(metadata.description)) ?? decode(first(metadata.creator));
  return files.map((file, index) => ({
    id: files.length === 1 ? identifier : `${identifier}/${file.name}`,
    title: files.length === 1 ? title : unshouted(fileTitle(file)) || `${title} (${index + 1})`,
    artist: performer ? peopleOf(performer) : null,
    album: files.length === 1 ? album : title,
    durationMs: lengthMs(file.length),
    artworkUrl: `${API}/services/img/${encodeURIComponent(identifier)}`,
    source: {
      kind: 'url',
      url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
      headers: {},
    },
  }));
};

/** The records found, as songs in the search's order, read a few at a time. */
const sides = async (identifiers: string[]): Promise<Track[]> => {
  const found: Track[] = [];
  for (let at = 0; at < identifiers.length; at += AT_ONCE) {
    const batch = await Promise.allSettled(identifiers.slice(at, at + AT_ONCE).map(recordingsOf));
    for (const result of batch) {
      if (result.status === 'fulfilled') found.push(...result.value);
      else raygun.log.warn('record skipped:', String(result.reason));
    }
  }
  return found;
};

/** Identifiers of the sides the advanced search finds (all of them for no `q`), most played first. */
const searchSides = async (q: string, rows: number): Promise<string[]> => {
  const where = q ? `(${q}) AND ${allowed()}` : allowed();
  const params = [
    `q=${encodeURIComponent(where)}`,
    'fl[]=identifier',
    `rows=${rows}`,
    `sort[]=${encodeURIComponent('downloads desc')}`,
    'output=json',
  ];
  const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params.join('&')}`);
  return (answer.response?.docs ?? []).map((doc) => doc.identifier).filter(Boolean);
};

// ---- Charts ----

type Listed = { chart: Chart; q: string };

const genre = (id: string, name: string, tag: string | null, values: string[]): Listed => ({
  chart: { id: `genre:${id}`, name, tag, group: 'genre' },
  q: `genre:(${values.map((value) => `"${value}"`).join(' OR ')})`,
});

const decade = (start: number): Listed => {
  const end = Math.min(start + 9, cutoff());
  return {
    chart: {
      id: `decade:${start}`,
      name: end < start + 9 ? `${start}–${end}` : `The ${start}s`,
      group: 'decade',
    },
    q: `year:[${start} TO ${end}]`,
  };
};

const listed = (): Listed[] => [
  {
    chart: {
      id: 'top',
      name: 'Most played 78s',
      description: 'The records listened to most on the Internet Archive',
      group: 'top',
    },
    q: '',
  },
  decade(1890),
  decade(1900),
  decade(1910),
  decade(1920),
  genre('jazz', 'Early jazz', 'Jazz', ['Jazz']),
  genre('blues', 'Early blues', 'Blues', ['Blues']),
  genre('ragtime', 'Ragtime', null, ['Ragtime', 'Cakewalk', 'Two-Step', 'One-Step']),
  genre('popular', 'Popular songs', null, ['Popular Music']),
  genre('opera', 'Opera and operetta', 'Classical', ['Opera', 'Operetta']),
  genre('classical', 'Classical', 'Classical', ['Classical', 'Orchestral']),
  genre('dance', 'Dance bands', null, ['Fox Trot', 'Fox-Trot', 'Foxtrot', 'Waltz', 'Polka']),
  genre('comedy', 'Comedy and novelty', null, [
    'Comedy',
    'Comic Song',
    'Comic',
    'Novelty',
    'Monologue',
    'Sketch',
    'Laughing Record',
  ]),
  genre('folk', 'Folk and old time', 'Folk', [
    'Folk',
    'Hillbilly',
    'Old Time',
    'Irish',
    'Yodeling',
  ]),
  genre('sacred', 'Sacred songs', null, ['Sacred', 'Religious', 'Hymn', 'Gospel', 'Spiritual']),
  genre('hawaiian', 'Hawaiian', null, ['Hawaiian']),
  genre('latin', 'Tango and Latin', 'Latin', ['Tango', 'Latin']),
  genre('marches', 'Marches and bands', null, ['March', 'Military', 'Band']),
  genre('broadway', 'Broadway and musicals', null, ['Broadway', 'Musical']),
  genre('christmas', 'Christmas', null, ['Christmas']),
];

/** Raygun's genres (and their other spellings) these records answer, by chart. */
const TAGS: Record<string, string> = {
  jazz: 'genre:jazz',
  dixieland: 'genre:jazz',
  blues: 'genre:blues',
  ragtime: 'genre:ragtime',
  classical: 'genre:classical',
  opera: 'genre:opera',
  folk: 'genre:folk',
  country: 'genre:folk',
  'old time': 'genre:folk',
  gospel: 'genre:sacred',
  christian: 'genre:sacred',
  latin: 'genre:latin',
  tango: 'genre:latin',
  'show tunes': 'genre:broadway',
  musicals: 'genre:broadway',
  christmas: 'genre:christmas',
};

const lists = new Map<string, { at: number; tracks: Promise<Track[]> }>();

/** A chart's songs, kept for half an hour; rows shown unasked leave out titles with slurs. */
const listOf = ({ chart, q }: Listed): Promise<Track[]> => {
  const kept = lists.get(chart.id);
  if (kept && Date.now() - kept.at < LIST_TTL_MS) return kept.tracks;
  const current = {
    at: Date.now(),
    tracks: searchSides(q, LIST_MOST + 10)
      .then(sides)
      .then((tracks) => tracks.filter((track) => !SLUR.test(track.title)).slice(0, LIST_MOST)),
  };
  current.tracks.catch(() => {
    if (lists.get(chart.id) === current) lists.delete(chart.id);
  });
  lists.set(chart.id, current);
  return current.tracks;
};

const listById = (id: string) => {
  const found = listed().find((each) => each.chart.id === id);
  if (!found) throw new Error('The Great 78 Project has no such chart');
  return found;
};

registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'records',
    name: 'Great 78 Project',

    async search({ query, limit }) {
      // Each word as a prefix: titles keep their possessives ("Alexander's Ragtime Band").
      const words = escapeQuery(query)
        .split(/\s+/)
        .filter(Boolean)
        .map((word) => (word.length >= 3 ? `${word}*` : word))
        .join(' ');
      if (!words) return { tracks: [] };
      const rows = Math.max(1, Math.min(limit, 12));
      const found = await searchSides(`title:(${words}) OR creator:(${words})`, rows);
      return { tracks: (await sides(found)).slice(0, limit) };
    },
  }),
);

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'Great 78 Project',

    charts: async () => ({ charts: listed().map((each) => each.chart) }),

    async chart({ id, limit }) {
      const tracks = await listOf(listById(id));
      return { tracks: tracks.slice(0, Math.max(1, limit)) };
    },

    async tagSongs({ tag, aliases, kind, limit }) {
      if (kind !== 'genre') return { tracks: [] };
      const id = [tag, ...aliases]
        .map((name) => TAGS[name.toLowerCase()])
        .find((found): found is string => Boolean(found));
      if (!id) return { tracks: [] };
      const found = listById(id);
      const tracks = await listOf(found);
      return { tracks: tracks.slice(0, Math.max(1, limit)), name: `${found.chart.name} on 78s` };
    },
  }),
);
