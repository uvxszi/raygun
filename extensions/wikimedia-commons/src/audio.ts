// Songs from Commons' audio files: search, and streams for songs found elsewhere. MIDI files and
// tiny files (pronunciations, sound effects) are left out; each song plays from Commons' MP3 copy
// of the file, or the file itself when it has none.
import { defineProvider } from '@raygun/sdk';
import type { MediaSource, Track } from '@raygun/sdk';

import {
  MEDIA_HEADERS,
  adult,
  clean,
  creatorOf,
  nameOf,
  openlyLicensed,
  searchFiles,
  titleOf,
} from './commons';
import type { CommonsFile, FileInfo } from './commons';

/** Audio files Commons can play and that aren't a sound or a word (sizes in kilobytes). */
const AUDIO = 'filetype:audio -filemime:audio/midi filesize:>300';
/** Shorter recordings are sound effects and spoken words, not songs. */
const MIN_SECONDS = 30;
const PLAYABLE = /^(audio\/(mpeg|ogg|flac|x-flac|wav|x-wav|webm|opus)|application\/ogg)/;

// Search syntax would turn a listener's words into a different search.
const words = (text: string) =>
  text
    .replace(/[\\"~*?:()[\]{}^|/-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

/** "Hello Dolly (Live) [Remastered]" → "Hello Dolly": file titles rarely carry the extras. */
const bareTitle = (title: string) => title.replace(/\([^)]*\)|\[[^\]]*\]/g, ' ').trim() || title;

/** "Ella Fitzgerald & Louis Armstrong" → "Ella Fitzgerald". */
const mainArtist = (artist: string) =>
  artist.split(/\s*(?:&|,|;|\/|\bfeat\.?|\bft\.?|\band\b|\bwith\b)\s*/i)[0]?.trim() || artist;

const wordsOf = (text: string) => words(text).toLowerCase().split(' ').filter(Boolean);

type Stream = { source: MediaSource; quality: string; bitrateKbps: number | null };

/** What plays: Commons' MP3 copy, else the file when players read its format. */
const streamOf = (info: FileInfo): Stream | null => {
  const mp3 = info.derivatives?.find((copy) => copy.transcodekey === 'mp3' && copy.src);
  if (mp3) {
    return {
      source: { kind: 'url', url: mp3.src, headers: MEDIA_HEADERS },
      quality: 'MP3',
      bitrateKbps: mp3.bandwidth ? Math.round(mp3.bandwidth / 1000) : null,
    };
  }
  const url = clean(info.url);
  if (!url || !PLAYABLE.test(info.mime ?? '')) return null;
  const format = /flac/.test(info.mime ?? '')
    ? 'FLAC'
    : (info.mime ?? '').split('/')[1]?.toUpperCase();
  return {
    source: { kind: 'url', url, headers: MEDIA_HEADERS },
    quality: format ?? 'Audio',
    bitrateKbps: format === 'FLAC' ? 1411 : null,
  };
};

type Song = { track: Track; stream: Stream };

/** Commons files as songs: openly licensed, long enough, playable, for everyone unless asked. */
const songs = (files: CommonsFile[]): Song[] =>
  files.flatMap((file) => {
    const info = file.videoinfo?.[0];
    if (!info || info.mediatype !== 'AUDIO' || /midi/.test(info.mime ?? '')) return [];
    if (!openlyLicensed(info) || (adult(info) && !raygun.content.showNsfw())) return [];
    if ((info.duration ?? 0) < MIN_SECONDS) return [];
    const stream = streamOf(info);
    if (!stream) return [];
    return [
      {
        stream,
        track: {
          id: nameOf(file),
          title: titleOf(file),
          artist: creatorOf(info),
          album: null,
          durationMs: Math.round((info.duration ?? 0) * 1000),
          artworkUrl: null,
          source: stream.source,
        },
      },
    ];
  });

export const search = defineProvider({
  kind: 'music.metadata',
  id: 'audio',
  name: 'Wikimedia Commons',

  async search({ query, limit }) {
    const asked = words(query);
    if (asked === '') return { tracks: [] };
    const { files } = await searchFiles(`${AUDIO} ${asked}`, { limit: Math.min(50, limit * 2) });
    return {
      tracks: songs(files)
        .slice(0, limit)
        .map((song) => song.track),
    };
  },
});

// Streams for songs found elsewhere: recordings whose title has every word of the song's title.
// Commons mostly holds classical and traditional music, folk songs and anthems; Raygun ranks what
// comes back.
export const streams = defineProvider({
  kind: 'music.stream',
  id: 'stream',
  name: 'Wikimedia Commons',

  async candidates({ title, artist, limit }) {
    const wanted = wordsOf(bareTitle(title));
    if (wanted.length === 0) return { candidates: [] };
    const named = words(bareTitle(title));
    const by = artist ? words(mainArtist(artist)) : '';
    // Pages don't always name the performer or composer, so the title alone is asked too.
    const searches = by ? [`${AUDIO} ${named} ${by}`, `${AUDIO} ${named}`] : [`${AUDIO} ${named}`];
    const answers = await Promise.allSettled(
      searches.map((search) => searchFiles(search, { limit: 20 })),
    );
    const failures = answers.flatMap((answer) =>
      answer.status === 'rejected' ? [answer.reason] : [],
    );
    if (failures.length === answers.length) throw failures[0];
    const files = [
      ...new Map(
        answers
          .flatMap((answer) => (answer.status === 'fulfilled' ? answer.value.files : []))
          .map((file) => [file.title, file]),
      ).values(),
    ];
    const matching = songs(files).filter(({ track }) => {
      const offered = wordsOf(track.title);
      return wanted.every((word) => offered.includes(word));
    });
    return {
      candidates: matching.slice(0, limit).map(({ track, stream }) => ({
        id: String(track.id),
        title: track.title,
        artist: track.artist ?? null,
        durationMs: track.durationMs ?? null,
        quality: stream.quality,
        bitrateKbps: stream.bitrateKbps,
        source: stream.source,
      })),
    };
  },
});
