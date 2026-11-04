import { ArrowDownCircle, ArrowDownToLine, CircleCheck } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSongSaves } from '../collections/savedSongs';
import { trackKey } from './trackKey';
import { useDownloadedKeys, useDownloadingKeys } from './useSongDownloads';

/**
 * A song's marks beside its title, in every list (M46.45): saved (in Favorites or a playlist,
 * under whatever names it was saved with, M46.48), downloaded, and on its way (M46.46). The shell
 * listens for download changes once for all.
 */
export const SongMarks: FC<{
  title: string;
  artist: string | null | undefined;
  /** The key a saved row already has, checked too. */
  trackKey?: string;
}> = ({ title, artist, trackKey: known }) => {
  const { t } = useTranslation();
  const saves = useSongSaves({ title, artist }, known);
  const downloaded = useDownloadedKeys(false);
  const downloading = useDownloadingKeys();
  const keys = [trackKey(title, artist), ...(known ? [known] : [])];
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
      {/* On its way (M46.46): queued, downloading or being recorded. */}
      {!keys.some((key) => downloaded.has(key)) && keys.some((key) => downloading.has(key)) && (
        <ArrowDownToLine
          role="img"
          aria-label={t('songs.downloading')}
          size={13}
          className="shrink-0 animate-pulse text-sky-500"
        />
      )}
      {keys.some((key) => downloaded.has(key)) && (
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
