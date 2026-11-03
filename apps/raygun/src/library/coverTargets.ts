import type { DetailTarget } from '@raygun/bindings';

/** A song's cover target, for `fallback`. */
export const songCover = (track: {
  title: string;
  artist: string | null;
  album?: string | null;
  durationMs?: number | null;
}): DetailTarget => ({
  kind: 'song',
  query: {
    title: track.title,
    artist: track.artist,
    album: track.album ?? null,
    durationMs: track.durationMs ?? null,
  },
});

/** An album's cover target, for `fallback`. */
export const albumCover = (album: { title: string; artist?: string | null }): DetailTarget => ({
  kind: 'album',
  album: album.title,
  artist: album.artist ?? null,
});

/** An artist's picture target, for `fallback`. */
export const artistCover = (name: string): DetailTarget => ({ kind: 'artist', name });
