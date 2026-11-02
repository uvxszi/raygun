import type { QueueItem } from '@raygun/bindings';
import type { FC } from 'react';

import { SaveButton } from '../collections/SaveButton';
import { fromQueueItem } from '../collections/useCollections';

/**
 * Saves the current song in the player bar, side panel and full player: Spotify's Plus, which
 * adds to Favorites and opens Add to playlist (M46.8).
 */
export const FavoriteButton: FC<{ item: QueueItem; compact?: boolean }> = ({ item, compact }) => (
  <SaveButton track={fromQueueItem(item)} compact={compact} />
);
