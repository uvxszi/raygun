import { useCurrentItem, useIsActive } from './usePlayback';

/** Case, accents and punctuation don't decide whether two names are the same song. */
const simple = (text: string | null | undefined) =>
  (text ?? '')
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/**
 * Whether a listed song is the one playing (M46.16): `'playing'`, `'paused'`, or `null` for
 * another song. Lists match by title and artist, since a song shows up from many sources.
 *
 * Every row of a list calls this, so it only follows the playing song and whether it plays:
 * position reports never re-render the rows.
 */
export const usePlayingMatch = () => {
  const item = useCurrentItem();
  const active = useIsActive();
  const title = simple(item?.title);
  const artist = simple(item?.artist);
  const status = active ? 'playing' : 'paused';
  return (song: { title: string; artist?: string | null }): 'playing' | 'paused' | null => {
    if (!item || !title || simple(song.title) !== title) return null;
    const other = simple(song.artist);
    if (artist && other && !other.includes(artist) && !artist.includes(other)) return null;
    return status;
  };
};
