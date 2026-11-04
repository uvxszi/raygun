import type { OnlineTrack, SourceChoice, TrackInput } from '@raygun/bindings';

/** The details a picked source can give a song (M46.48). */
export type DetailField = 'title' | 'artist' | 'album' | 'cover';

export const DETAIL_FIELDS: DetailField[] = ['title', 'artist', 'album', 'cover'];

/** One of the song's details beside the picked source's. */
export type Difference = { field: DetailField; song: string | null; source: string };

const text = (value: string | null | undefined) => (value ?? '').trim();

/**
 * The details the picked source has and the song shows differently: title, artist and album as
 * text, the cover when the source has one. Nothing when the names agree.
 */
export const differences = (song: TrackInput, track: OnlineTrack): Difference[] => {
  const found: Difference[] = [];
  const named = (field: DetailField, mine: string | null | undefined, theirs: string | null) => {
    if (text(theirs) && text(theirs) !== text(mine)) {
      found.push({ field, song: text(mine) || null, source: text(theirs) });
    }
  };
  named('title', song.title, track.title);
  named('artist', song.artist, track.artist);
  named('album', song.album, track.album);
  // Covers only with names to change: the same picture often has another address.
  if (found.length > 0 && track.artworkUrl && track.artworkUrl !== song.artworkUrl) {
    found.push({ field: 'cover', song: song.artworkUrl, source: track.artworkUrl });
  }
  return found;
};

/** What confirming a pick sends: the stream (unless downloading) and the details ticked. */
export const sourceChoice = (
  assign: boolean,
  sourceName: string | undefined,
  ticked: ReadonlySet<DetailField>,
): SourceChoice => ({
  assign,
  sourceName: sourceName ?? null,
  title: ticked.has('title'),
  artist: ticked.has('artist'),
  album: ticked.has('album'),
  cover: ticked.has('cover'),
});
