import type { ContinueWatching } from '@raygun/bindings';
import type { FC } from 'react';

import { useMangaLibrary } from '../manga/useManga';
import { storedTitle, stremioParts } from './continueEntry';
import { ServerPicker } from './ServerPicker';
import { StreamPicker } from './StreamPicker';
import { STREMIO_SOURCE } from './useVideoLibrary';

/** The source picker for an entry, as its title's page opens it: addons' streams or servers. */
export const ContinueSourcePicker: FC<{ item: ContinueWatching; onClose: () => void }> = ({
  item,
  onClose,
}) => {
  const library = useMangaLibrary('video');
  if (item.sourceId === STREMIO_SOURCE && item.remoteId && item.episodeId) {
    const { kind, id } = stremioParts(item.remoteId);
    // A saved title's downloads count toward it in the library.
    const saved = library.data?.find(
      (entry) => entry.sourceId === STREMIO_SOURCE && entry.remoteId === item.remoteId,
    );
    return (
      <StreamPicker
        target={{
          kind,
          metaId: id,
          videoId: item.episodeId,
          title: item.title,
          subtitle: item.subtitle,
          poster: item.poster,
          titleId: saved?.titleId ?? null,
        }}
        onClose={onClose}
      />
    );
  }
  const titleId = storedTitle(item);
  if (!item.sourceId || !titleId || !item.unitId || !item.episodeId) return null;
  return (
    <ServerPicker
      target={{
        sourceId: item.sourceId,
        titleId,
        episodeId: item.unitId,
        remoteId: item.episodeId,
        heading: [item.title, item.subtitle].filter(Boolean).join(' · '),
      }}
      onClose={onClose}
    />
  );
};
