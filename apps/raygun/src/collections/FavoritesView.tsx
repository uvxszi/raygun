import { commands } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { savedRow } from '../music/tableView';
import { TrackTable } from '../music/TrackTable';
import { EmptyState } from '../shell/EmptyState';
import { PlayButtons } from './PlayButtons';
import { COLLECTIONS, playCollection, useFavorites } from './useCollections';

export const FavoritesView: FC = () => {
  const { t } = useTranslation();
  const favorites = useFavorites();
  const client = useQueryClient();
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
      <TrackTable
        kind="favorites"
        tracks={rows}
        customLabel={t('tracks.sort.addedOrder')}
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
