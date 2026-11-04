import { events } from '@raygun/bindings';
import type { QueryClient, QueryKey } from '@tanstack/react-query';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

/** What shows watching progress: Continue watching, history, title pages and the video library. */
export const WATCH_PROGRESS_KEYS: QueryKey[] = [
  ['watch'],
  ['video', 'title'],
  ['video', 'library'],
  ['video', 'updates'],
  ['video', 'upcoming'],
  ['stremio', 'meta'],
];

/** What shows podcast and audiobook progress, downloads and refreshes. */
export const PODCAST_KEYS: QueryKey[] = [['podcasts'], ['audiobooks']];

/** What lists sources: they follow installs, removals and extension settings. */
export const SOURCE_KEYS: QueryKey[] = [
  ['manga', 'sources'],
  ['novel', 'sources'],
  ['video', 'sources'],
  ['music', 'charts'],
  ['music', 'tags'],
  ['podcasts', 'top'],
  ['lyricsOptions'],
];

const refresh = (client: QueryClient, keys: QueryKey[]) => {
  for (const queryKey of keys) void client.invalidateQueries({ queryKey });
};

/**
 * The one place Rust's change events reach the cached answers (M46.48): each event refreshes
 * every view showing what changed, whichever screens are open. Hooks that keep an event's payload
 * (update status, the extension list) still listen for it themselves. Mount once, in the shell.
 */
export const useLiveInvalidation = () => {
  const client = useQueryClient();
  useEffect(() => {
    const listening = [
      // The player saved progress, finished something or closed.
      events.watchChangedEvent.listen(() => refresh(client, WATCH_PROGRESS_KEYS)),
      // A refresh, a download, a finished or listened episode.
      events.podcastsChangedEvent.listen(() => refresh(client, PODCAST_KEYS)),
      events.extensionsChangedEvent.listen(() => refresh(client, SOURCE_KEYS)),
      // Another device's changes may touch anything shown.
      events.syncChangedEvent.listen(() => void client.invalidateQueries()),
    ];
    return () => {
      for (const unlisten of listening) void unlisten.then((stop) => stop());
    };
  }, [client]);
};
