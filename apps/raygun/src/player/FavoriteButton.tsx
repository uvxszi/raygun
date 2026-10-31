import type { QueueItem } from '@raygun/bindings';
import { Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem, useIsFavorite, useSetFavorite } from '../collections/useCollections';

/** Hearts the current track in the full player. */
export const FavoriteButton: FC<{ item: QueueItem }> = ({ item }) => {
  const { t } = useTranslation();
  const track = fromQueueItem(item);
  const favorite = useIsFavorite(track);
  const setFavorite = useSetFavorite();
  const isFavorite = favorite.data === true;
  return (
    <button
      type="button"
      aria-label={t(isFavorite ? 'player.unfavorite' : 'player.favorite')}
      aria-pressed={isFavorite}
      disabled={favorite.isPending || setFavorite.isPending}
      onClick={() => setFavorite.mutate({ track, favorite: !isFavorite })}
      className={`grid size-11 shrink-0 place-items-center rounded-full ${isFavorite ? 'text-primary' : 'text-muted'}`}
    >
      <Heart aria-hidden size={22} fill={isFavorite ? 'currentColor' : 'none'} />
    </button>
  );
};
