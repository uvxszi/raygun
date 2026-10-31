import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlayButtons } from './PlayButtons';
import { SavedTrackRow } from './SavedTrackRow';
import { playCollection, useFavorites } from './useCollections';

export const FavoritesView: FC = () => {
  const { t } = useTranslation();
  const favorites = useFavorites();
  if (!favorites.data) {
    return null;
  }
  if (favorites.data.length === 0) {
    return <p className="text-muted text-sm">{t('collections.noFavorites')}</p>;
  }
  return (
    <div data-testid="favorites-view" className="flex flex-col gap-3">
      <PlayButtons
        onPlay={(shuffle) =>
          void playCollection({ collection: { kind: 'favorites' }, startIndex: null, shuffle })
        }
      />
      <ul className="flex flex-col">
        {favorites.data.map((track, index) => (
          <li key={track.trackKey}>
            <SavedTrackRow
              track={track}
              onPlay={() =>
                void playCollection({
                  collection: { kind: 'favorites' },
                  startIndex: index,
                  shuffle: false,
                })
              }
            />
          </li>
        ))}
      </ul>
    </div>
  );
};
