import type { FC } from 'react';

import { useCollectionSheets } from './collectionSheets';
import { ImportSheet } from './ImportSheet';
import { MergePlaylistsSheet } from './MergePlaylistsSheet';

/** Hosts Import songs and Merge playlists for the whole app (M46.45). */
export const CollectionSheets: FC = () => {
  const { importing, merging, close } = useCollectionSheets();
  if (importing) return <ImportSheet onClose={close} />;
  if (merging) return <MergePlaylistsSheet picked={merging} onClose={close} />;
  return null;
};
