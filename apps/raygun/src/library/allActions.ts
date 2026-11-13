import { commands } from '@raygun/bindings';
import type { PodcastPicks, TrackInput } from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';

import { COLLECTIONS, fromSavedTrack } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';
import { downloadSongs } from '../music/useSongDownloads';
import { QUICK_ACCESS_KEY } from '../shell/railPrefs';
import type { AllItem } from './allItems';

/**
 * What Library → All's selection does to the picked items, kind by kind, through each kind's own
 * commands (the services in Rust do the work): playlists' songs, shows and books by their picks,
 * comics, books and videos by medium, sites one by one.
 */

const titlesByMedium = (items: AllItem[]) => {
  const groups = new Map<Medium, string[]>();
  for (const item of items) {
    if (item.kind === 'title') {
      groups.set(item.medium, [...(groups.get(item.medium) ?? []), item.entry.titleId]);
    }
  }
  return [...groups];
};

/** The picked shows and audiobooks, each standing for its episodes or chapters. */
const showPicks = (items: AllItem[]): PodcastPicks => ({
  shows: items.flatMap((item) =>
    item.kind === 'podcast' ? [item.show.id] : item.kind === 'audiobook' ? [item.book.id] : [],
  ),
  episodes: [],
});

/** The picked playlists' songs, in the order shown. */
export const playlistSongs = async (items: AllItem[]): Promise<TrackInput[]> => {
  const lists = await Promise.all(
    items.flatMap((item) =>
      item.kind === 'playlist' ? [unwrapResult(commands.playlistGet(item.playlist.id))] : [],
    ),
  );
  return lists.flatMap((list) => list.items.map((entry) => fromSavedTrack(entry.track)));
};

/** Plays the picked playlists', shows' and books' songs and episodes next or last: how many. */
export const queueAll = async (items: AllItem[], next: boolean) => {
  const songs = await playlistSongs(items);
  const picks = showPicks(items);
  const queueSongs = async () => {
    if (songs.length === 0) return 0;
    await unwrapResult(commands.playbackQueueTracks(songs, next));
    return songs.length;
  };
  const queueShows = async () =>
    picks.shows.length > 0 ? unwrapResult(commands.podcastQueuePicks(picks, next)) : 0;
  // Each "next" goes in ahead of the one before: shows first, so the songs still play first.
  if (next) {
    const episodes = await queueShows();
    return episodes + (await queueSongs());
  }
  const queued = await queueSongs();
  return queued + (await queueShows());
};

/** Marks the picked titles read or watched, and shows and books played or finished. */
export const markAll = async (items: AllItem[], done: boolean) => {
  const picks = showPicks(items);
  await Promise.all([
    ...titlesByMedium(items).map(([medium, ids]) =>
      unwrapResult(commands.libraryMarkRead(medium, ids, done)),
    ),
    ...(picks.shows.length > 0 ? [unwrapResult(commands.podcastSetPlayedPicks(picks, done))] : []),
  ]);
};

/**
 * Downloads the picked playlists' songs, shows' and books' episodes, and the titles' next
 * `amount` chapters or episodes (all unread or unwatched when null): how many were queued.
 */
export const downloadAll = async (items: AllItem[], amount: number | null) => {
  const songs = await playlistSongs(items);
  const picks = showPicks(items);
  const counts = await Promise.all([
    songs.length > 0 ? downloadSongs(songs).then(() => songs.length) : 0,
    picks.shows.length > 0 ? unwrapResult(commands.podcastDownloadPicks(picks)) : 0,
    ...titlesByMedium(items).map(([medium, ids]) =>
      unwrapResult(commands.libraryDownloadNext(medium, ids, amount)),
    ),
  ]);
  return counts.reduce((sum, count) => sum + count, 0);
};

/** Adds the picked titles to the library's favorites, or takes them out. */
export const favoriteAll = (items: AllItem[], favorite: boolean) =>
  Promise.all(
    titlesByMedium(items).map(([medium, ids]) =>
      unwrapResult(commands.librarySetFavorite(medium, ids, favorite)),
    ),
  );

/**
 * Takes the picked items out of the library: playlists and sites are deleted, shows unfollowed,
 * books and titles removed (with their downloads when `deleteDownloads`). Every kind is tried;
 * returns how many left and how many couldn't.
 */
export const removeAll = async (items: AllItem[], deleteDownloads: boolean) => {
  const picks = showPicks(items);
  const jobs: { count: number; run: () => Promise<unknown> }[] = [
    ...items.flatMap((item) =>
      item.kind === 'playlist'
        ? [{ count: 1, run: () => unwrapResult(commands.playlistDelete(item.playlist.id)) }]
        : item.kind === 'website'
          ? [{ count: 1, run: () => unwrapResult(commands.websiteRemove(item.site.id)) }]
          : [],
    ),
    ...(picks.shows.length > 0
      ? [
          {
            count: picks.shows.length,
            run: () => unwrapResult(commands.podcastSetSubscribedMany(picks.shows, false)),
          },
        ]
      : []),
    ...titlesByMedium(items).map(([medium, ids]) => ({
      count: ids.length,
      run: () => unwrapResult(commands.libraryRemove(medium, ids, true, deleteDownloads)),
    })),
  ];
  const settled = await Promise.allSettled(jobs.map((job) => job.run()));
  const failed = settled.reduce(
    (sum, result, index) => sum + (result.status === 'rejected' ? (jobs[index]?.count ?? 0) : 0),
    0,
  );
  return { removed: items.length - failed, failed };
};

/** Everything that lists what changed: the library's kinds and quick access. */
export const refreshAll = (client: QueryClient, items: AllItem[]) => {
  const media = new Set(items.flatMap((item) => (item.kind === 'title' ? [item.medium] : [])));
  return Promise.all([
    client.invalidateQueries({ queryKey: [COLLECTIONS] }),
    client.invalidateQueries({ queryKey: ['podcasts'] }),
    client.invalidateQueries({ queryKey: ['audiobooks'] }),
    client.invalidateQueries({ queryKey: ['websites'] }),
    client.invalidateQueries({ queryKey: QUICK_ACCESS_KEY }),
    ...[...media].map((medium) => client.invalidateQueries({ queryKey: [medium, 'library'] })),
  ]);
};
