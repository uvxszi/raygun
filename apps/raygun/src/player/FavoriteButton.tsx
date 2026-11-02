import type { QueueItem } from '@raygun/bindings';
import { Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem, useIsFavorite, useSetFavorite } from '../collections/useCollections';

/** Hearts the current track in the full player and the player bar (`compact`). */
export const FavoriteButton: FC<{ item: QueueItem; compact?: boolean }> = ({ item, compact }) => {
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
      className={`grid shrink-0 place-items-center rounded-full ${compact ? 'size-9' : 'size-11'} ${isFavorite ? 'text-primary' : 'text-muted'}`}
    >
      <Heart aria-hidden size={compact ? 18 : 22} fill={isFavorite ? 'currentColor' : 'none'} />
    </button>
  );
};
