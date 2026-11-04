import { ArrowDownCircle, ArrowDownToLine, CircleCheck } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSongSaves } from '../collections/savedSongs';
import { PULSE } from './songDownloadState';
import { keysOf } from './trackKey';
import { useSongDownloadState } from './useSongDownloads';

/**
 * A song's marks beside its title, in every list (M46.45): saved (in Favorites or a playlist,
 * under whatever names it was saved with, M46.48), downloaded, and on its way (M46.46): waiting,
 * or downloading now (it pulses). The marks read the one list of saved songs and the one list of
 * downloads, and change only when this song's entries do.
 */
export const SongMarks: FC<{
  title: string;
  artist: string | null | undefined;
  /** The key a saved row already has, checked too. */
  trackKey?: string;
}> = ({ title, artist, trackKey: known }) => {
  const { t } = useTranslation();
  const saves = useSongSaves({ title, artist }, known);
  // A download made under the title with or without its " - Remastered" (M46.48).
  const keys = [...keysOf(title, artist), ...(known ? [known] : [])];
  const download = useSongDownloadState(keys);
  return (
    <>
      {saves?.saved && (
        <CircleCheck
          role="img"
          aria-label={t('songs.saved')}
          size={13}
          className="text-primary shrink-0"
        />
      )}
      {download === 'queued' && (
        <ArrowDownToLine
          role="img"
          aria-label={t('songs.waiting')}
          size={13}
          className="text-muted shrink-0"
        />
      )}
      {(download === 'downloading' || download === 'recording') && (
        <ArrowDownToLine
          role="img"
          aria-label={t('songs.downloading')}
          size={13}
          className={`shrink-0 text-sky-500 ${PULSE}`}
        />
      )}
      {download === 'done' && (
        <ArrowDownCircle
          role="img"
          aria-label={t('songs.downloaded')}
          size={13}
          className="shrink-0 text-sky-500"
        />
      )}
    </>
  );
};
