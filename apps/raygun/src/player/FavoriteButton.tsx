import type { QueueItem } from '@raygun/bindings';
import type { FC } from 'react';

import { SaveButton } from '../collections/SaveButton';
import { fromQueueItem } from '../collections/useCollections';
import { DownloadSongButton } from '../music/DownloadSongButton';

/**
 * Saves the current song in the player bar, side panel and full player: Spotify's Plus, which
 * adds to Favorites and opens Add to playlist (M46.8), and with `download`, the download button
 * beside it (M46.18).
 */
export const FavoriteButton: FC<{ item: QueueItem; compact?: boolean; download?: boolean }> = ({
  item,
  compact,
  download = false,
}) => {
  const track = fromQueueItem(item);
  return (
    <>
      {download && <DownloadSongButton track={track} compact={compact} />}
      <SaveButton track={track} compact={compact} />
    </>
  );
};
