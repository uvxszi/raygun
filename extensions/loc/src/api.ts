// The loc.gov JSON API, kept within its limits. The API allows 20 requests a minute and blocks a
// client that asks more for an hour (it answers 429, or a web page with a challenge, instead of
// JSON), so every request waits its turn: one at a time, at least 3.1 s after the last one
// started. A block is kept in storage and nothing is asked until it ends. Uncached queries
// sometimes stall until the request times out and then answer at once, so a failed request is
// asked once more (taking its turn again, and late enough to find it answered). Answers are kept
// in memory: lists and items by the modules that read them, the most recent few hundred.

const SITE = 'https://www.loc.gov';
const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';
/** Between the starts of two requests: 20 a minute is one per 3 s. */
const GAP_MS = 3100;
/** How long the API shuts a client out once it asked too much. */
const BLOCK_MS = 60 * 60 * 1000;
const BLOCKED_KEY = 'blockedUntil';

/** A map that forgets the entries used least recently past `size`. */
export class Lru<V> {
  private readonly entries = new Map<string, V>();

  constructor(private readonly size: number) {}

  get(key: string): V | undefined {
    const value = this.entries.get(key);
    if (value !== undefined) {
      this.entries.delete(key);
      this.entries.set(key, value);
    }
    return value;
  }

  set(key: string, value: V): void {
    this.entries.delete(key);
    this.entries.set(key, value);
    while (this.entries.size > this.size) {
      const oldest = this.entries.keys().next().value;
      if (oldest === undefined) break;
      this.entries.delete(oldest);
    }
  }
}

/** `raygun.time.sleep` waits at most 10 s a call. */
const pause = async (ms: number) => {
  let left = ms;
  while (left > 0) {
    const step = Math.min(left, 10_000);
    await raygun.time.sleep(step);
    left -= step;
  }
};

let lastStart = 0;
let line: Promise<unknown> = Promise.resolve();

/** Runs `request` in its turn: after every request asked before it, `GAP_MS` after the last. */
const inTurn = <T>(request: () => Promise<T>): Promise<T> => {
  const turn = line.then(async () => {
    const wait = lastStart + GAP_MS - Date.now();
    if (wait > 0) await pause(wait);
    lastStart = Date.now();
    return request();
  });
  line = turn.catch(() => undefined);
  return turn;
};

const blockedUntil = (): number => {
  const until = raygun.storage.get<number>(BLOCKED_KEY);
  return typeof until === 'number' && until > Date.now() ? until : 0;
};

const blockedError = (until: number) => {
  const minutes = Math.max(1, Math.ceil((until - Date.now()) / 60_000));
  return new Error(
    `The Library of Congress asked Raygun to slow down; it can be asked again in ${minutes} minute${minutes === 1 ? '' : 's'}`,
  );
};

/** What a request met: an answer, a block, or a failure worth one more try. */
type Outcome<T> = { answer: T } | { blocked: true } | { missing: true } | { retry: string };

const once = async <T>(url: string): Promise<Outcome<T>> => {
  let response;
  try {
    response = await raygun.http.fetch(url, {
      headers: { Accept: 'application/json', 'User-Agent': AGENT },
    });
  } catch (error) {
    return { retry: String(error) };
  }
  const type = response.headers['content-type'] ?? '';
  if (response.status === 429) return { blocked: true };
  if (response.status === 404) return { missing: true };
  if (response.status >= 500) return { retry: `HTTP ${response.status}` };
  // A web page where JSON was asked for is the API's challenge to a client it shut out.
  if (type.includes('text/html')) return { blocked: true };
  if (!response.ok) throw new Error(`The Library of Congress answered HTTP ${response.status}`);
  try {
    return { answer: await response.json<T>() };
  } catch (error) {
    // Stalled answers end cut off.
    return { retry: String(error) };
  }
};

/** Whole answers, a few: lists and items keep what they show in caches of their own. */
const answers = new Lru<unknown>(16);
/** Requests under way, so the same page asked twice at once is one request. */
const asking = new Map<string, Promise<unknown>>();

/**
 * A request that stalled until it timed out is still being worked out on the Library's side, and
 * the same request answers at once when it's done, about 45 s after the first. A second request
 * sooner stalls as well, so the retry starts no sooner than this after the first.
 */
const STALL_RETRY_MS = 30_000;
/** A request that took this long stalled (requests time out after 20 s). */
const STALLED_MS = 15_000;

const ask = async <T>(url: string): Promise<T | null> => {
  let firstStart = 0;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const until = blockedUntil();
    if (until) throw blockedError(until);
    if (attempt > 0 && Date.now() - firstStart >= STALLED_MS) {
      await pause(firstStart + STALL_RETRY_MS - Date.now());
    }
    const outcome = await inTurn(() => {
      if (attempt === 0) firstStart = Date.now();
      return once<T>(url);
    });
    if ('answer' in outcome) {
      answers.set(url, outcome.answer);
      return outcome.answer;
    }
    if ('missing' in outcome) return null;
    if ('blocked' in outcome) {
      const end = Date.now() + BLOCK_MS;
      raygun.storage.set(BLOCKED_KEY, end);
      throw blockedError(end);
    }
    raygun.log.warn(`loc.gov didn't answer ${url}: ${outcome.retry}`);
  }
  throw new Error('The Library of Congress is slow to answer; try again');
};

/**
 * A JSON answer from loc.gov (`path` with its query, `fo=json` added), from memory when it was
 * asked lately. `null` when the API says there is no such page.
 */
export const locJson = async <T>(path: string): Promise<T | null> => {
  const url = `${SITE}${path}${path.includes('?') ? '&' : '?'}fo=json`;
  const kept = answers.get(url);
  if (kept !== undefined) return kept as T;
  let request = asking.get(url) as Promise<T | null> | undefined;
  if (!request) {
    request = ask<T>(url).finally(() => asking.delete(url));
    asking.set(url, request);
  }
  return request;
};

// ---- What the answers hold ----

export type Resource = {
  caption?: string;
  video?: string;
  video_stream?: string;
  poster?: string;
  image?: string;
  audio?: string;
  duration?: number;
  height?: number;
  width?: number;
};

/** A search result: an item as collections and searches list it. */
export type ListedItem = {
  id?: string;
  url?: string;
  title?: string;
  date?: string;
  access_restricted?: boolean;
  partof?: string[];
  subject?: string[];
  image_url?: string[];
  resources?: Resource[];
  /** In lists, `contributors` are names with roles: "Collins, Arthur, 1864-1933. prf". */
  item?: { contributors?: unknown[] };
};

export type Listing = {
  results?: ListedItem[];
  pagination?: { current?: number; next?: string | null; total?: number; of?: number };
};

/** An item's own page (`at=item,resources`). */
export type ItemAnswer = {
  item?: {
    title?: string;
    date?: string;
    access_restricted?: boolean;
    partof?: (string | { title?: string })[];
    rights?: string | string[];
    rights_advisory?: string | string[];
    summary?: string[];
    description?: string[];
    /** "Paley, William Daly, 1857-1924, camera" (`contributors` here are search links). */
    contributor_names?: unknown[];
    genre?: string[];
    subject_headings?: string[];
    created_published?: string[];
    url?: string;
  };
  resources?: Resource[];
};

// ---- The public-domain rules ----

const thisYear = () => new Date().getUTCFullYear();
/**
 * The last year whose US works are in the public domain: their copyright ran 95 years from
 * publication, to the end of the year (in 2026, works from 1930 and before).
 */
export const filmCutoff = () => thisYear() - 96;
/**
 * The last year whose US sound recordings are: 100 years from publication, to the end of the year,
 * for recordings from 1923 to 1946; older ones already are (in 2026, recordings from 1925 and
 * before).
 */
export const recordingCutoff = () => thisYear() - 101;

/**
 * The latest year a date can mean: "1898" is 1898, "191?" 1919, "1898-1904" 1904. `null`
 * without a year.
 */
export const latestYear = (date: string | undefined): number | null => {
  const years = (date ?? '').match(/\b1[89]\d[\d?]|\b20\d[\d?]/g) ?? [];
  const numbers = years.map((year) => Number(year.replace('?', '9')));
  return numbers.length > 0 ? Math.max(...numbers) : null;
};

/** The first year a date names, for showing. */
export const firstYear = (date: string | undefined): number | null => {
  const year = /\b(1[89]\d\d|20\d\d)\b/.exec(date ?? '');
  return year ? Number(year[1]) : null;
};

/** The National Jukebox's recordings are lent by their rights holder, not public domain. */
export const jukebox = (partof: (string | { title?: string })[] | undefined) =>
  (partof ?? []).some((part) =>
    /national jukebox/i.test(typeof part === 'string' ? part : (part.title ?? '')),
  );

/** An item's id from its address: `http://www.loc.gov/item/98501077/` is `98501077`. */
export const itemId = (listed: ListedItem): string | null => {
  const match = /\/item\/([^/?#]+)\/?/.exec(listed.url ?? listed.id ?? '');
  return match?.[1] ?? null;
};

/** `//tile.loc.gov/…` addresses as https. */
export const https = (address: string | undefined): string | null => {
  if (!address) return null;
  if (address.startsWith('//')) return `https:${address}`;
  return /^https?:\/\//i.test(address) ? address.replace(/^http:/i, 'https:') : null;
};

const text = (value: string | string[] | undefined) =>
  (Array.isArray(value) ? value.join(' ') : (value ?? ''))
    .replace(/<[^>]+>/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();

/** Rights statements that say the Library knows of no copyright or other restriction. */
const FREE =
  /not aware of any u\.?\s?s\.? copyright|no known (copyright )?restrictions|(is|are) in the public domain|free to use and reuse/i;
/** Advisories that say otherwise. */
const RESTRICTED =
  /rights status not evaluated|copyright(ed)? (protected|restricted|held)|may be (protected|restricted)|permission (is )?required|restricted access|for educational (purposes|use) only/i;

/**
 * Whether an item's own rights statement lets Raygun offer it: its advisory (when it has one)
 * names no restriction, and the statement or advisory says the Library knows of none.
 */
export const freeToUse = (item: ItemAnswer['item']): boolean => {
  const advisory = text(item?.rights_advisory);
  if (advisory && RESTRICTED.test(advisory)) return false;
  return FREE.test(advisory) || FREE.test(text(item?.rights));
};

/** The credit line the rights statement asks for, from its own paragraph. */
export const creditLine = (item: ItemAnswer['item']): string | null => {
  const rights = item?.rights;
  const html = Array.isArray(rights) ? rights.join('\n') : (rights ?? '');
  for (const paragraph of html.split(/<\/p>|<br\s*\/?>|\r?\n/i)) {
    const credit = /credit line:\s*([^.]+)/i.exec(text(paragraph));
    if (credit?.[1]) return credit[1].trim();
  }
  return null;
};

// ---- Names ----

/** Roles the catalogue writes in brackets: "Sousa's Band (Performer)". */
const BRACKETED_ROLE =
  /\s*\((performer|composer|conductor|singer|speaker|lyricist|arranger|narrator)\)\s*$/i;
/** Relator codes after the name: "Collins, Arthur, 1864-1933. prf". */
const CODES: Record<string, string> = {
  prf: 'performer',
  spk: 'speaker',
  sng: 'singer',
  voc: 'singer',
  itr: 'performer',
  lyr: 'lyricist',
  cnd: 'conductor',
  cmp: 'composer',
  arr: 'arranger',
  aut: 'author',
  drt: 'director',
  pro: 'producer',
};
const CORPORATE =
  /^(inc|co|corp|ltd|llc|company|firm|bros|brothers|limited|incorporated|and company|& co)\b/i;
/** Roles the catalogue writes in lower case after a name, with or without a comma. */
const KNOWN_ROLE =
  /(?:,\s*|\s+)(former owner|production company|film distributor|copyright claimant|camera|cinematographer|director|producer|actor|performer|animator|scenarist|writer|writing|editor|narrator|composer|singer|speaker|lyricist|conductor|arranger|donor)\.?$/;
/** Other roles are lower case after the name: "sponsor", "film editor". */
const LOWER_CASE = /^[a-z][a-z .&'/-]*$/;
const LIFE_DATES = /^(?:ca\.\s*)?(?:\d{3,4}\??)?-?(?:\d{3,4}\??)?$/;

/**
 * A contributor as the catalogue writes it, without its dates, fuller forms and roles:
 * "Collins, Arthur, 1864-1933. prf" is Arthur Collins, a performer; "International Film Service,
 * production company, film distributor" is International Film Service, a production company;
 * "Thomas A. Edison, Inc" stays as it is.
 */
export const contributor = (raw: string): { name: string; role: string | null } => {
  let rest = raw.trim();
  const roles: string[] = [];
  const bracketed = BRACKETED_ROLE.exec(rest);
  if (bracketed) {
    roles.push(bracketed[1]!.toLowerCase());
    rest = rest.slice(0, bracketed.index);
  }
  // A relator code after a full stop or a space: ". prf".
  const code = /(?:\.\s*|\s+)([a-z]{3})\.?$/.exec(rest);
  if (code && CODES[code[1]!]) {
    roles.unshift(CODES[code[1]!]!);
    rest = rest.slice(0, code.index);
    // "Humphrey, Harry E. prf": the full stop was the initial's.
    if (/(?:^|[\s,])[A-Z]$/.test(rest)) rest += '.';
  }
  rest = rest.replace(/\s*\([^)]*\)/g, '').trim();
  // "Paramount Pictures Corporation former owner", "Fulton, Helen, 1894- actor".
  for (let role = KNOWN_ROLE.exec(rest); role; role = KNOWN_ROLE.exec(rest)) {
    roles.unshift(role[1]!);
    rest = rest.slice(0, role.index).trim();
  }
  const parts = rest
    .split(/\s*,\s*/)
    .map((part) => part.trim())
    .filter(Boolean);
  while (parts.length > 1 && LOWER_CASE.test(parts[parts.length - 1]!)) {
    roles.unshift(parts.pop()!);
  }
  const named = parts.filter((part) => !(/\d/.test(part) && LIFE_DATES.test(part)));
  let name = named.join(', ');
  if (named.length >= 2 && !CORPORATE.test(named[1]!)) {
    name = [`${named[1]} ${named[0]}`, ...named.slice(2)].join(', ');
  }
  // A full stop that ends the name, but not an initial's.
  if (name.endsWith('.') && !/\b[A-Z]\.$/.test(name)) name = name.slice(0, -1);
  return { name: name.replace(/\s+/g, ' ').trim(), role: roles[0] ?? null };
};

/** Collections and former owners are listed among contributors; they made nothing. */
export const madeIt = ({ name, role }: { name: string; role: string | null }) =>
  name !== '' &&
  role !== 'former owner' &&
  role !== 'donor' &&
  !/collection|library of congress/i.test(name);

/** Plain text of the catalogue's HTML-ish fields. */
export const plain = text;
