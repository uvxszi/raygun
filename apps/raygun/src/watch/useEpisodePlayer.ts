import type { VideoUpdateItem } from '@raygun/bindings';
import { useState } from 'react';

import { STREMIO_SOURCE, useWatchSourceTitle, useWatchStremio } from './useVideoLibrary';

/**
 * Plays episodes of saved titles through a session (M44: fresh links, the last source first).
 * One per list: the row played last shows its wait and what went wrong.
 */
export const useEpisodePlayer = () => {
  const source = useWatchSourceTitle();
  const stremio = useWatchStremio();
  const [last, setLast] = useState<string | null>(null);
  const mine = (item: VideoUpdateItem) => last === item.episodeId;
  return {
    play: (item: VideoUpdateItem) => {
      setLast(item.episodeId);
      source.reset();
      stremio.reset();
      if (item.sourceId === STREMIO_SOURCE) {
        const split = item.remoteId.indexOf(':');
        stremio.mutate({
          kind: item.remoteId.slice(0, split),
          id: item.remoteId.slice(split + 1),
          videoId: item.episodeRemoteId,
        });
      } else {
        source.mutate({ titleId: item.titleId, episodeId: item.episodeId });
      }
    },
    pending: (item: VideoUpdateItem) => mine(item) && (source.isPending || stremio.isPending),
    error: (item: VideoUpdateItem) => (mine(item) ? (source.error ?? stremio.error) : null),
  };
};

export type EpisodePlayer = ReturnType<typeof useEpisodePlayer>;
