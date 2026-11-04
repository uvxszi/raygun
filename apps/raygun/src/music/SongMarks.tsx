import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { ArrowDownCircle, ArrowDownToLine, CircleCheck } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { COLLECTIONS } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import { PULSE } from './songDownloadState';
import { trackKey } from './trackKey';
import { useSongDownloadState } from './useSongDownloads';

/** Keys of every saved song (Favorites or a playlist), refreshed with the collections. */
const useSavedKeys = () => {
  const keys = useQuery({
    queryKey: [COLLECTIONS, 'savedKeys'],
    queryFn: () => unwrapResult(commands.collectionsSavedKeys()),
    staleTime: 60_000,
  });
  return new Set(keys.data ?? []);
};

/**
 * A song's marks beside its title, in every list (M46.45): saved (in Favorites or a playlist),
 * downloaded, and on its way (M46.46): waiting, or downloading now (it pulses). The marks read
 * the one list of downloads and change only when this song's download does.
 */
export const SongMarks: FC<{
  title: string;
  artist: string | null | undefined;
  /** The key a saved row already has, checked too. */
  trackKey?: string;
}> = ({ title, artist, trackKey: known }) => {
  const { t } = useTranslation();
  const saved = useSavedKeys();
  const keys = [trackKey(title, artist), ...(known ? [known] : [])];
  const download = useSongDownloadState(keys);
  return (
    <>
      {keys.some((key) => saved.has(key)) && (
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
