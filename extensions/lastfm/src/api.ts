// Last.fm's web service (ws.audioscrobbler.com/2.0) with the person's own API key, and what its
// JSON answers share. Within Last.fm's API terms: requests take turns a quarter of a second apart
// (Last.fm asks for no more than five a second), a rate limit is waited out, and answers are kept
// in memory as long as their HTTP headers allow (an hour when they say nothing), a few megabytes
// in all, far below the 100 MB the terms allow.
import { needsSetup, query } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const API = 'https://ws.audioscrobbler.com/2.0';
const HEADERS = {
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
  Accept: 'application/json',
};
/** Time between the starts of two requests. */
const SPACING_MS = 250;
/** How long an answer is kept when its headers don't say, and at most. */
const KEEP_MS = 60 * 60 * 1000;
const KEEP_AT_MOST_MS = 24 * 60 * 60 * 1000;
/** All kept answers together, at most, counted in characters of their JSON. */
const KEEP_CHARS = 8 * 1024 * 1024;
/** How long nothing is asked after Last.fm says requests come too fast. */
const REST_MS = 60 * 1000;
const RATE_LIMITED = 'Last.fm is limiting requests for a moment; try again in a minute';

/** The person's key; without one the source needs setup. */
const apiKey = (): string => {
  const key = raygun.settings.get('apiKey');
  const typed = typeof key === 'string' ? key.trim() : '';
  if (!typed) {
    return needsSetup(
      "Last.fm needs your own free API key: create one at last.fm/api/account/create and enter it in this source's settings",
    );
  }
  return typed;
};

// ---- Turns ----

let nextStart = 0;
let restUntil = 0;

/** Waits for this request's turn: requests start a quarter of a second apart. */
const turn = async () => {
  const start = Math.max(Date.now(), nextStart);
  nextStart = start + SPACING_MS;
  for (let wait = start - Date.now(); wait > 0; wait = start - Date.now()) {
    await raygun.time.sleep(Math.min(wait, 10_000));
  }
};

// ---- Kept answers ----

type Kept = { until: number; size: number; value: unknown };
const answers = new Map<string, Kept>();
let keptChars = 0;

const forget = (key: string) => {
  const known = answers.get(key);
  if (!known) return;
  keptChars -= known.size;
  answers.delete(key);
};

const keep = (key: string, value: unknown, size: number, ms: number) => {
  forget(key);
  if (ms <= 0 || size > KEEP_CHARS / 8) return;
  answers.set(key, { until: Date.now() + ms, size, value });
  keptChars += size;
  // The oldest go first.
  while (keptChars > KEEP_CHARS && answers.size > 0) forget(answers.keys().next().value ?? key);
};

/** How long an answer's `Cache-Control` lets it be kept. */
const keepFor = (headers: Record<string, string>) => {
  const control = (headers['cache-control'] ?? '').toLowerCase();
  if (/no-store|no-cache/.test(control)) return 0;
  const maxAge = /max-age=(\d+)/.exec(control);
  return maxAge ? Math.min(Number(maxAge[1]) * 1000, KEEP_AT_MOST_MS) : KEEP_MS;
};

// ---- Requests ----

type Failure = { error?: unknown; message?: unknown };
type Params = Record<string, string | number | null | undefined>;

/**
 * Last.fm's answer to `method`, or `null` when Last.fm doesn't know what was asked about (its
 * error 6: no such artist, song or country). The key goes last in the address and never into an
 * error.
 */
export const lastfm = async <T>(method: string, params: Params): Promise<T | null> => {
  const key = apiKey();
  const asked = query({ method, ...params, format: 'json' });
  const known = answers.get(asked);
  if (known && known.until > Date.now()) return known.value as T | null;
  if (Date.now() < restUntil) throw new Error(RATE_LIMITED);
  await turn();
  // Last.fm may have asked for a rest while this request waited its turn.
  if (Date.now() < restUntil) throw new Error(RATE_LIMITED);
  const response = await raygun.http.fetch(`${API}/?${asked}&${query({ api_key: key })}`, {
    headers: HEADERS,
  });
  const text = await response.text();
  let answer: (T & Failure) | null = null;
  try {
    const parsed: unknown = JSON.parse(text);
    if (parsed && typeof parsed === 'object') answer = parsed as T & Failure;
  } catch {
    answer = null;
  }
  const code = answer?.error === undefined ? null : Number(answer.error);
  const said = typeof answer?.message === 'string' ? answer.message.trim() : '';
  if (code === 26) {
    needsSetup(
      "Last.fm has suspended this API key: create a new one at last.fm/api/account/create and enter it in this source's settings",
    );
  }
  if (code === 10 || response.status === 401 || response.status === 403) {
    needsSetup("Last.fm didn't accept the API key: check it in this source's settings");
  }
  if (code === 29 || response.status === 429) {
    restUntil = Date.now() + REST_MS;
    throw new Error(RATE_LIMITED);
  }
  if (code === 6) {
    keep(asked, null, 16, keepFor(response.headers));
    return null;
  }
  if (code === 8 || code === 11 || code === 16) {
    throw new Error('Last.fm is having trouble answering right now; try again in a few minutes');
  }
  if (code !== null) throw new Error(`Last.fm couldn't answer${said ? `: ${said}` : ''}`);
  if (!response.ok) throw new Error(`Last.fm answered with an error (HTTP ${response.status})`);
  if (!answer) throw new Error("Last.fm's answer couldn't be read");
  keep(asked, answer, text.length, keepFor(response.headers));
  return answer;
};

// ---- Answers ----

/** A list Last.fm gives as one object when it holds one item, and may leave out when empty. */
export const listOf = <T>(value: T | T[] | null | undefined): T[] =>
  Array.isArray(value) ? value : value && typeof value === 'object' ? [value] : [];

/** Text as given, spaces tidied; anything else is empty. */
export const textOf = (value: unknown): string =>
  typeof value === 'string' ? value.replace(/\s+/g, ' ').trim() : '';

/** Numbers come as numbers or as text. */
export const numberOf = (value: unknown): number | null => {
  const number =
    typeof value === 'number'
      ? value
      : typeof value === 'string'
        ? Number(value.trim() || NaN)
        : NaN;
  return Number.isFinite(number) ? number : null;
};

export const clamp = (value: number, low: number, high: number) =>
  Math.max(low, Math.min(Number.isFinite(value) ? Math.floor(value) : low, high));

/** Lower-case words without accents or punctuation, for comparing names. */
export const words = (text: string) =>
  text
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

export type LastfmArtist = { name?: unknown; '#text'?: unknown; mbid?: unknown; url?: unknown };

export type LastfmTrack = {
  name?: unknown;
  mbid?: unknown;
  url?: unknown;
  /** Seconds, as text or a number; 0 when Last.fm doesn't know. */
  duration?: unknown;
  artist?: LastfmArtist | string | null;
};

/** A track list's `track`, wherever the answer keeps it. */
export type TrackList = { track?: LastfmTrack | LastfmTrack[] | null } | null | undefined;

const artistName = (artist: LastfmTrack['artist']) =>
  typeof artist === 'string' ? textOf(artist) : textOf(artist?.name) || textOf(artist?.['#text']);

/**
 * A length in milliseconds: lists give seconds, track details milliseconds (no song in a list
 * lasts 30,000 seconds, so a number that large is milliseconds already).
 */
const lengthOf = (duration: unknown) => {
  const number = numberOf(duration);
  if (!number || number <= 0) return null;
  return Math.round(number >= 30_000 ? number : number * 1000);
};

/** A Last.fm song as Raygun's: no artwork (Last.fm's images aren't for other apps). */
export const toTrack = (track: LastfmTrack): Track[] => {
  const title = textOf(track.name);
  if (!title) return [];
  const artist = artistName(track.artist);
  return [
    {
      id: textOf(track.mbid) || textOf(track.url) || `${artist} – ${title}`,
      title,
      artist: artist || null,
      durationMs: lengthOf(track.duration),
    },
  ];
};

/** Songs without repeats of a song (the same title and artist, spelled alike). */
export const distinct = (tracks: Track[]) => {
  const seen = new Set<string>();
  return tracks.filter((track) => {
    const key = `${words(track.artist ?? '')}|${words(track.title)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

/** The songs of a track list, without repeats. */
const tracksOf = (list: TrackList): Track[] => distinct(listOf(list?.track).flatMap(toTrack));

/** An answer holding songs: charts' `tracks`, an artist's `toptracks`, a song's `similartracks`. */
export type TrackAnswer = {
  tracks?: TrackList;
  toptracks?: TrackList;
  similartracks?: TrackList;
} | null;

/** The songs of an answer, whichever list it names. */
export const songsIn = (answer: TrackAnswer): Track[] =>
  tracksOf(answer?.tracks ?? answer?.toptracks ?? answer?.similartracks);
