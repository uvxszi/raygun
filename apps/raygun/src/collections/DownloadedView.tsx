import { Link } from '@tanstack/react-router';
import { CircleArrowDown, Settings2 } from 'lucide-react';
import { useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { savedRow } from '../music/tableView';
import { TrackTable } from '../music/TrackTable';
import {
  useDownloadedSongs,
  useSongDownloadActions,
  useSongDownloadsSelect,
} from '../music/useSongDownloads';
import { EmptyState } from '../shell/EmptyState';
import { PlayButtons } from './PlayButtons';
import { playCollection } from './useCollections';

/**
 * Library → Audio → Downloaded (M46.18): every song downloaded into the app, whether or not it's
 * in Favorites or a playlist. They play from their files; removing one deletes its download.
 */
export const DownloadedView: FC = () => {
  const { t } = useTranslation();
  const songs = useDownloadedSongs();
  // The finished downloads' ids by song; other songs' progress doesn't redraw the table.
  const pairs = useSongDownloadsSelect((list) =>
    list
      .filter((item) => item.state === 'done')
      .map((item) => [item.trackKey, item.id] as const)
      .sort(([a], [b]) => a.localeCompare(b)),
  ).data;
  const idFor = useMemo(() => new Map(pairs ?? []), [pairs]);
  const { removeMany } = useSongDownloadActions();
  if (!songs.data) {
    return null;
  }
  if (songs.data.length === 0) {
    return (
      <EmptyState
        icon={CircleArrowDown}
        title={t('library.downloaded.emptyTitle')}
        body={t('library.downloaded.emptyHint')}
      />
    );
  }
  const rows = songs.data.map((track) => savedRow(track, track.trackKey));
  return (
    <div data-testid="downloaded-view" className="flex flex-col gap-3">
      <TrackTable
        kind="downloaded"
        tracks={rows}
        customLabel={t('tracks.sort.addedOrder')}
        toolbar={
          <div className="flex items-center gap-2">
            <PlayButtons
              onPlay={(shuffle) =>
                void playCollection({
                  collection: { kind: 'downloaded' },
                  startIndex: null,
                  shuffle,
                })
              }
            />
            <Link
              to="/downloads"
              search={{ tab: 'music' }}
              className="text-muted hover:text-foreground flex items-center gap-1.5 rounded-full px-3 py-2 text-sm"
            >
              <Settings2 aria-hidden size={16} />
              {t('library.downloaded.manage')}
            </Link>
          </div>
        }
        onPlay={(ordered, index) =>
          void playCollection({
            collection: { kind: 'downloaded' },
            startIndex: index,
            shuffle: false,
            order: ordered.map((row) => row.trackKey),
          })
        }
        onRemove={(removed) => {
          const ids = removed.flatMap((row) => idFor.get(row.trackKey) ?? []);
          if (ids.length > 0) removeMany.mutate(ids);
        }}
      />
    </div>
  );
};
