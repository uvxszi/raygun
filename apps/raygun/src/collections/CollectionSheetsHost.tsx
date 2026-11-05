import type { FC } from 'react';

import { useCollectionSheets } from './collectionSheets';
import { ImportSheet } from './ImportSheet';
import { MergePlaylistsSheet } from './MergePlaylistsSheet';

/** Hosts Import songs and Merge playlists for the whole app (M46.45). */
export const CollectionSheets: FC = () => {
  const { importing, link, merging, close } = useCollectionSheets();
  // A new link opens the sheet afresh.
  if (importing) return <ImportSheet key={link} initialLink={link} onClose={close} />;
  if (merging) return <MergePlaylistsSheet picked={merging} onClose={close} />;
  return null;
};
