import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { Heart } from 'lucide-react';
import type { SavedTrack } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { joinCount, useSongsLength } from '../music/collectionLength';
import { usePlayContext } from '../music/playContext';
import { savedRow } from '../music/tableView';
import { TrackTable } from '../music/TrackTable';
import { EmptyState } from '../shell/EmptyState';
import { PlayButtons } from './PlayButtons';
import { COLLECTIONS, playCollection, useFavorites, useReorderFavorites } from './useCollections';

const NO_SONGS: SavedTrack[] = [];

export const FavoritesView: FC = () => {
  const { t } = useTranslation();
  const favorites = useFavorites();
  const reorder = useReorderFavorites();
  const client = useQueryClient();
  const length = useSongsLength(favorites.data ?? NO_SONGS);
  usePlayContext({
    kind: 'favorites',
    name: t('collections.favorites'),
    subtitle: null,
    artworkUrl: null,
    to: '/library',
    search: { view: 'favorites' },
  });
  if (!favorites.data) {
    return null;
  }
  if (favorites.data.length === 0) {
    return (
      <EmptyState
        icon={Heart}
        title={t('collections.noFavoritesTitle')}
        body={t('collections.noFavorites')}
      />
    );
  }
  const rows = favorites.data.map((track) => savedRow(track, track.trackKey));
  return (
    <div data-testid="favorites-view" className="flex flex-col gap-3">
      <p className="text-muted text-xs">
        {joinCount(t('library.trackCount', { count: rows.length }), length)}
      </p>
      <TrackTable
        kind="favorites"
        tracks={rows}
        onReorder={(change) => reorder.mutate(change)}
        toolbar={
          <PlayButtons
            onPlay={(shuffle) =>
              void playCollection({ collection: { kind: 'favorites' }, startIndex: null, shuffle })
            }
          />
        }
        // A sorted or filtered table plays in the order it shows (M46.8).
        onPlay={(ordered, index) =>
          void playCollection({
            collection: { kind: 'favorites' },
            startIndex: index,
            shuffle: false,
            order: ordered.map((row) => row.trackKey),
          })
        }
        onRemove={(removed) =>
          void Promise.all(
            removed.map((row) => unwrapResult(commands.favoriteSet(row.input, false))),
          ).then(() => client.invalidateQueries({ queryKey: [COLLECTIONS] }))
        }
      />
    </div>
  );
};
