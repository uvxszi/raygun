// Sound recordings from the Library of Congress: the Edison company's discs and the Variety Stage
// collection's recordings, whose rights statements say the Library knows of no copyright or other
// restriction, published more than 100 years ago (US recordings that old are in the public domain).
// Other collections' statements reserve rights, and the National Jukebox's recordings are lent by
// their rights holder, so neither is used. There are fewer than a hundred, so both lists are read
// once (two requests), kept a week, and searched on the device; each collection is also an album.
// A disc's sides are separate recordings.
import { defineProvider, query } from '@raygun/sdk';
import type { Album, Track } from '@raygun/sdk';

import {
  contributor,
  firstYear,
  https,
  itemId,
  jukebox,
  latestYear,
  locJson,
  madeIt,
  recordingCutoff,
} from './api';
import type { Listing, ListedItem } from './api';

type Collection = { id: string; slug: string; title: string; words: string };

const COLLECTIONS: Collection[] = [
  {
    id: 'edison',
    slug: 'edison-company-motion-pictures-and-sound-recordings',
    title: 'Inventing Entertainment: Edison sound recordings',
    words: 'edison inventing entertainment diamond disc records',
  },
  {
    id: 'variety',
    slug: 'variety-stage-sound-recordings-and-motion-pictures',
    title: 'Variety Stage sound recordings',
    words: 'variety stage vaudeville records',
  },
];
const KEEP_MS = 7 * 24 * 60 * 60 * 1000;
const STORE_KEY = 'recordings';
/** Bump when what's kept changes, so older lists are read again. */
const STORE_VERSION = 1;

type Recording = {
  id: string;
  title: string;
  artist: string | null;
  year: number | null;
  url: string;
  album: string;
  subjects: string;
};
type Stored = { version: number; at: number; recordings: Recording[] };

const folded = (value: string) => value.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase();
const wordsOf = (value: string) =>
  folded(value)
    .split(/[^a-z0-9]+/)
    .filter(Boolean);

/** "Hungarian rag (Edison 50123-L)" → "Hungarian rag". */
const sideTitle = (caption: string | undefined) => {
  const title = (caption ?? '')
    .replace(/\s*\((?:edison|victor|columbia|berliner)\b[^)]*\)\s*$/i, '')
    .trim();
  return /digital audio|^online\b/i.test(title) ? '' : title;
};

const capitalized = (title: string) => title.charAt(0).toUpperCase() + title.slice(1);

/** Who is heard: the performers, else whoever the catalogue names first. */
const artistOf = (listed: ListedItem): string | null => {
  const people = (listed.item?.contributors ?? [])
    .filter((name): name is string => typeof name === 'string')
    .map(contributor)
    .filter(madeIt);
  const heard = people.filter(
    (person) => person.role && /perform|speak|sing|conduct/.test(person.role),
  );
  const credited = heard.length > 0 ? heard : people.slice(0, 1);
  const names = [...new Set(credited.map((person) => person.name))];
  return names.length > 0 ? names.slice(0, 3).join(' & ') : null;
};

const recordingsOf = (listed: ListedItem, album: string): Recording[] => {
  const id = itemId(listed);
  const year = latestYear(listed.date);
  if (!id || year === null || year > recordingCutoff()) return [];
  if (listed.access_restricted === true || jukebox(listed.partof)) return [];
  const sides = (listed.resources ?? []).filter((resource) => https(resource.audio));
  return sides.flatMap((resource, index) => {
    const title = sideTitle(resource.caption) || (sides.length === 1 ? listed.title?.trim() : '');
    if (!title) return [];
    return [
      {
        id: `${id}#${index}`,
        title: capitalized(title),
        artist: artistOf(listed),
        year: firstYear(listed.date),
        url: https(resource.audio)!,
        album,
        subjects: (listed.subject ?? []).join(' '),
      },
    ];
  });
};

const readCollection = async (collection: Collection): Promise<Recording[]> => {
  const found: Recording[] = [];
  for (let page = 1; page <= 3; page += 1) {
    const answer = await locJson<Listing>(
      `/collections/${collection.slug}/?${query({
        fa: 'online-format:audio',
        at: 'results,pagination',
        c: 100,
        sp: page,
      })}`,
    );
    for (const listed of answer?.results ?? []) found.push(...recordingsOf(listed, collection.id));
    if (!answer?.pagination?.next) break;
  }
  return found;
};

let kept: Recording[] | null = null;
let reading: Promise<Recording[]> | null = null;

const read = async (): Promise<Recording[]> => {
  const lists = [];
  for (const collection of COLLECTIONS) lists.push(await readCollection(collection));
  // A recording in both collections is the Edison one.
  const byUrl = new Map<string, Recording>();
  for (const recording of lists.flat()) {
    if (!byUrl.has(recording.url)) byUrl.set(recording.url, recording);
  }
  const recordings = [...byUrl.values()];
  try {
    raygun.storage.set(STORE_KEY, { version: STORE_VERSION, at: Date.now(), recordings });
  } catch (error) {
    raygun.log.warn(`the recordings couldn't be kept: ${String(error)}`);
  }
  return recordings;
};

/**
 * Every recording, from memory, storage (for a week) or the Library; an older stored list stands
 * in while the Library can't be asked.
 */
const recordings = async (): Promise<Recording[]> => {
  if (kept) return kept;
  const stored = raygun.storage.get(STORE_KEY) as Stored | null;
  const usable =
    stored?.version === STORE_VERSION &&
    Array.isArray(stored.recordings) &&
    stored.recordings.length > 0
      ? stored
      : null;
  if (usable && Date.now() - usable.at < KEEP_MS) {
    kept = usable.recordings;
    return kept;
  }
  reading ??= read().finally(() => {
    reading = null;
  });
  try {
    kept = await reading;
  } catch (error) {
    if (!usable) throw error;
    kept = usable.recordings;
  }
  return kept;
};

const trackOf = (recording: Recording): Track => ({
  id: recording.id,
  title: recording.title,
  artist: recording.artist,
  album: COLLECTIONS.find((collection) => collection.id === recording.album)?.title ?? null,
  source: { kind: 'url', url: recording.url, headers: {} },
});

/** How well a recording answers the words: every word must be found; titles count most. */
const score = (recording: Recording, wanted: string[]) => {
  const title = wordsOf(recording.title);
  const artist = wordsOf(recording.artist ?? '');
  const rest = wordsOf(`${recording.subjects} ${recording.year ?? ''}`);
  let total = 0;
  for (const word of wanted) {
    const starts = (list: string[]) => list.some((found) => found.startsWith(word));
    if (title.includes(word)) total += 4;
    else if (starts(title)) total += 3;
    else if (artist.includes(word) || starts(artist)) total += 2;
    else if (starts(rest)) total += 1;
    else return 0;
  }
  return total;
};

const albumOf = (collection: Collection, all: Recording[]): Album => ({
  id: collection.id,
  title: collection.title,
  artist: 'Library of Congress',
  trackCount: all.filter((recording) => recording.album === collection.id).length,
  kind: 'compilation',
});

const byYear = (one: Recording, other: Recording) =>
  (one.year ?? 9999) - (other.year ?? 9999) || one.title.localeCompare(other.title);

export const recordingSearch = defineProvider({
  kind: 'music.metadata',
  id: 'recordings',
  name: 'Library of Congress',

  async search({ query: typed, limit }) {
    const wanted = wordsOf(typed);
    if (wanted.length === 0) return { tracks: [] };
    const found = (await recordings())
      .map((recording) => ({ recording, points: score(recording, wanted) }))
      .filter(({ points }) => points > 0)
      .sort((one, other) => other.points - one.points || byYear(one.recording, other.recording));
    return {
      tracks: found.slice(0, Math.max(1, limit)).map(({ recording }) => trackOf(recording)),
    };
  },

  async searchAlbums({ query: typed, limit }) {
    const wanted = wordsOf(typed);
    if (wanted.length === 0) return { albums: [] };
    const matching = COLLECTIONS.filter((collection) => {
      const words = wordsOf(`${collection.title} ${collection.words} library of congress`);
      return wanted.every((word) => words.some((found) => found.startsWith(word)));
    });
    if (matching.length === 0) return { albums: [] };
    const all = await recordings();
    return { albums: matching.slice(0, Math.max(1, limit)).map((found) => albumOf(found, all)) };
  },

  async album({ id }) {
    const collection = COLLECTIONS.find((found) => found.id === id);
    if (!collection) throw new Error('The Library of Congress has no such collection');
    const all = await recordings();
    return {
      album: albumOf(collection, all),
      tracks: all
        .filter((recording) => recording.album === id)
        .sort(byYear)
        .map(trackOf),
    };
  },
});
