import { events } from '@raygun/bindings';
import type { QueryClient, QueryKey } from '@tanstack/react-query';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { MIHON_RUNTIME } from '../extensions/useExtensions';
import { SOURCES_SETUP_KEY } from '../sources/useSourcesSetup';
import { QUICK_ACCESS_KEY } from './railPrefs';

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

/** The parts of an artist's page Rust keeps (`['music', part, name]`). */
const ARTIST_PARTS = new Set(['artist', 'artistInfo', 'artistReleases', 'artistEvents']);

/** Refreshes what shows the artist `name`'s page, whatever case its keys spell the name in. */
const refreshArtist = (client: QueryClient, name: string) => {
  const wanted = name.trim().toLowerCase();
  void client.invalidateQueries({
    predicate: ({ queryKey: [scope, part, who] }) =>
      scope === 'music' &&
      typeof part === 'string' &&
      ARTIST_PARTS.has(part) &&
      typeof who === 'string' &&
      who.trim().toLowerCase() === wanted,
  });
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
      // An artist's kept page was asked again in the background and changed.
      events.artistProfileEvent.listen((event) => refreshArtist(client, event.payload)),
      // Another device's changes may touch anything shown.
      events.syncChangedEvent.listen(() => void client.invalidateQueries()),
      // Quick access: the library or its settings changed, a page was played from.
      events.quickAccessChangedEvent.listen(() => refresh(client, [QUICK_ACCESS_KEY])),
      // Whether Mihon extensions' own code can run here (desktop's tools set up, or not).
      events.mihonRuntimeEvent.listen((event) => client.setQueryData(MIHON_RUNTIME, event.payload)),
      // Which sources wait for a key or sign-in (a load, a source saying its key was refused).
      events.sourcesSetupEvent.listen((event) =>
        client.setQueryData(SOURCES_SETUP_KEY, event.payload),
      ),
      // A finished scan brings the device's albums and artists.
      events.libraryScanEvent.listen((event) => {
        if (event.payload.phase === 'done' || event.payload.phase === 'failed') {
          refresh(client, [QUICK_ACCESS_KEY]);
        }
      }),
    ];
    return () => {
      for (const unlisten of listening) void unlisten.then((stop) => stop());
    };
  }, [client]);
};
