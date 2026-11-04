// The Commons API calls and file checks the songs and videos share.

export const COMMONS = 'https://commons.wikimedia.org';
const API = `${COMMONS}/w/api.php`;
const AGENT = 'Raygun/1.0 (https://github.com/uvxszi/raygun)';
// Wikimedia asks scripts to name themselves, the files' servers included; `Api-User-Agent` is
// the header for clients whose User-Agent is set for them.
export const MEDIA_HEADERS: Record<string, string> = { 'User-Agent': AGENT };
const API_HEADERS = { 'User-Agent': AGENT, 'Api-User-Agent': AGENT };

type MetaValue = { value?: string | number };
export type Derivative = {
  src: string;
  type?: string;
  transcodekey?: string;
  width?: number;
  height?: number;
  bandwidth?: number;
};
export type TimedText = { src: string; srclang?: string; label?: string; kind?: string };
export type FileInfo = {
  url?: string;
  thumburl?: string;
  mime?: string;
  mediatype?: string;
  width?: number;
  height?: number;
  duration?: number;
  size?: number;
  extmetadata?: Record<string, MetaValue>;
  derivatives?: Derivative[];
  timedtext?: TimedText[];
};
export type CommonsFile = {
  title: string;
  index?: number;
  missing?: boolean;
  videoinfo?: FileInfo[];
};
type QueryAnswer = {
  query?: { pages?: CommonsFile[]; searchinfo?: { totalhits?: number } };
  continue?: { gsroffset?: number };
};

/** The metadata fields read from each file's description page. */
const FIELDS = [
  'ObjectName',
  'ImageDescription',
  'Artist',
  'LicenseShortName',
  'License',
  'Categories',
  'DateTimeOriginal',
  'DateTime',
].join('|');

export const api = async <T>(params: Record<string, string | number>): Promise<T> => {
  const query = Object.entries({ format: 'json', formatversion: 2, ...params })
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}?${query}`, { headers: API_HEADERS });
  if (!response.ok) throw new Error(`Wikimedia Commons answered HTTP ${response.status}`);
  const answer = await response.json<T & { error?: { info?: string } }>();
  if (answer.error) {
    throw new Error(`Wikimedia Commons: ${answer.error.info ?? 'the request failed'}`);
  }
  return answer;
};

const fileProps = (withTracks: boolean) => ({
  prop: 'videoinfo',
  viprop: `url|size|mime|mediatype|extmetadata|derivatives${withTracks ? '|timedtext' : ''}`,
  viurlwidth: 480,
  viextmetadatafilter: FIELDS,
  viextmetadatamultilang: 0,
});

/**
 * Files found by a Commons search, in the search's order, with their details; `more` when the
 * search offers another page.
 */
export const searchFiles = async (
  search: string,
  { limit, offset = 0, sort = 'relevance' }: { limit: number; offset?: number; sort?: string },
): Promise<{ files: CommonsFile[]; more: boolean }> => {
  const answer = await api<QueryAnswer>({
    action: 'query',
    generator: 'search',
    gsrnamespace: 6,
    gsrsearch: search,
    gsrlimit: limit,
    gsroffset: offset,
    gsrsort: sort,
    ...fileProps(false),
  });
  const files = [...(answer.query?.pages ?? [])].sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
  return { files, more: Boolean(answer.continue?.gsroffset) };
};

/** One file by its name (without "File:"), with its subtitles. */
export const fileByName = async (name: string): Promise<CommonsFile> => {
  const answer = await api<QueryAnswer>({
    action: 'query',
    titles: `File:${name}`,
    ...fileProps(true),
  });
  const file = answer.query?.pages?.[0];
  if (!file || file.missing || !file.videoinfo?.[0]) {
    throw new Error('This file is no longer on Wikimedia Commons');
  }
  return file;
};

export const decode = (text: string) =>
  text
    .replace(/&nbsp;|&#160;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&apos;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(parseInt(code, 16)),
    )
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&');

export const plainText = (html: string) =>
  decode(
    html
      .replace(/<br\s*\/?>/gi, '\n')
      .replace(/<\/p>/gi, '\n\n')
      .replace(/<[^>]+>/g, ''),
  )
    // Spaces of any kind (no-break ones too), but not line breaks.
    .replace(/[^\S\n]+/g, ' ')
    .replace(/\n\s*\n\s*/g, '\n\n')
    .trim();

/** A metadata field as plain text ('' when the page has none). */
export const field = (info: FileInfo, key: string) => {
  const value = info.extmetadata?.[key]?.value;
  return value === undefined ? '' : plainText(String(value));
};

/** Public domain (marks, PD templates) or a Creative Commons licence, CC0 included. */
export const openlyLicensed = (info: FileInfo) =>
  /^(cc|pd)/i.test(field(info, 'License')) ||
  /^(CC[ -]|CC0|Public domain|PD\b)/i.test(field(info, 'LicenseShortName'));

/** Categories Commons files nudity and sex under. */
const ADULT =
  /\b(nude|nudes|nudity|nudist|nudism|naked|sex|sexual|sexuality|porn\w*|erotic\w*|genital\w*|penis|vagina|vulva|breasts?|nipples?|masturbat\w*|intercourse|fetish\w*|bdsm|striptease|topless)\b/i;
export const adult = (info: FileInfo) => ADULT.test(field(info, 'Categories'));

/** "File:Moonlight_Sonata.ogg" → "Moonlight_Sonata.ogg": the id the app keeps. */
export const nameOf = (file: CommonsFile) => file.title.replace(/^File:/, '');

/** The file's own title, else its name without the extension. */
export const titleOf = (file: CommonsFile) => {
  const info = file.videoinfo?.[0];
  const named = info ? field(info, 'ObjectName') : '';
  return (
    named ||
    nameOf(file)
      .replace(/\.[^.]+$/, '')
      .replace(/_/g, ' ')
  );
};

/**
 * Who made it, when the page names someone: the first line's first clause, without the words
 * before the names ("Produced by Pablo Vazquez, Beorn Leonard and Francesco Siddi. Soundtrack…").
 */
export const creatorOf = (info: FileInfo) => {
  const line = (field(info, 'Artist').split('\n')[0] ?? '')
    .split(/;|\.\s/)[0]!
    .replace(/^(credit|author|artist|by|produced by|directed by)\s*:?\s*/i, '')
    .trim();
  if (!line || /^(unknown|see above|see below|anonymous)/i.test(line) || line.length > 80) {
    return null;
  }
  return line;
};

/** Addresses without the tracking query Commons adds to them. */
export const clean = (url: string | undefined) =>
  url ? url.replace(/[?&]utm_[^#]*$/, '') : undefined;
