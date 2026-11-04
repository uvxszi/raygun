import { commands } from '@raygun/bindings';
import type { BulkResult, RemoteTitleRef } from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';
import type { TFunction } from 'i18next';

import { COLLECTIONS, fromOnlineTrack } from '../collections/useCollections';
import { unwrapResult } from '../lib/commandResult';
import type { Medium } from '../manga/medium';
import { downloadSongs } from '../music/useSongDownloads';
import type { PickedSong, PickedTitle } from './selectionStore';

/** The picked titles of each medium, as the bulk commands take them. */
export const byMedium = (titles: PickedTitle[]) => {
  const groups = new Map<Medium, RemoteTitleRef[]>();
  for (const title of titles) {
    const refs = groups.get(title.medium) ?? [];
    refs.push({ sourceId: title.sourceId, remoteId: title.remoteId, title: title.title });
    groups.set(title.medium, refs);
  }
  return [...groups];
};

/** Results of several bulk calls as one. */
const merged = (results: BulkResult[]): BulkResult => ({
  titleIds: results.flatMap((result) => result.titleIds),
  queued: results.reduce((sum, result) => sum + result.queued, 0),
  failed: results.flatMap((result) => result.failed),
});

/** Everything a library change shows in: libraries, groups, title pages, downloads. */
export const refreshLibraries = (client: QueryClient, media: Medium[]) =>
  Promise.all([
    ...media.flatMap((medium) =>
      ['library', 'categories', 'title', 'marks', 'downloads'].map((part) =>
        client.invalidateQueries({ queryKey: [medium, part] }),
      ),
    ),
    client.invalidateQueries({ queryKey: ['stremio'] }),
  ]);

/** Adds titles to the library, into `categoryIds` when given (one medium's groups or folders). */
export const addTitles = async (titles: PickedTitle[], categoryIds: string[] = []) =>
  merged(
    await Promise.all(
      byMedium(titles).map(([medium, refs]) =>
        unwrapResult(commands.libraryAddTitles(medium, refs, categoryIds)),
      ),
    ),
  );

/** Favorites songs and titles (titles join the library to be favorites). */
export const favoriteAll = async (
  client: QueryClient,
  songs: PickedSong[],
  titles: PickedTitle[],
) => {
  const results = await Promise.all(
    byMedium(titles).map(async ([medium, refs]) => {
      const added = await unwrapResult(commands.libraryAddTitles(medium, refs, []));
      if (added.titleIds.length > 0) {
        await unwrapResult(commands.librarySetFavorite(medium, added.titleIds, true));
      }
      return added;
    }),
  );
  if (songs.length > 0) {
    await unwrapResult(
      commands.favoritesSetMany(
        songs.map((song) => fromOnlineTrack(song.track)),
        true,
      ),
    );
    await client.invalidateQueries({ queryKey: [COLLECTIONS] });
  }
  return merged(results);
};

/**
 * Queues the next `amount` chapters or episodes of titles (all unread with none) and every song,
 * through their usual download queues.
 */
export const downloadAll = async (
  songs: PickedSong[],
  titles: PickedTitle[],
  amount: number | null,
) => {
  const results = await Promise.all(
    byMedium(titles).map(([medium, refs]) =>
      unwrapResult(commands.libraryDownloadTitles(medium, refs, amount)),
    ),
  );
  if (songs.length > 0) await downloadSongs(songs.map((song) => fromOnlineTrack(song.track)));
  const result = merged(results);
  return { ...result, queued: result.queued + songs.length };
};

/** What a bulk action did, in a line: what was done (none when nothing was), then what failed. */
export const resultMessage = (t: TFunction, result: BulkResult, done: string | null) => {
  const [first] = result.failed;
  const failure = !first
    ? null
    : result.failed.length > 1
      ? t('search.select.failed', { count: result.failed.length, title: first.title })
      : first.title
        ? t('search.select.failedSource', { title: first.title, message: first.message })
        : first.message;
  return [done, failure].filter(Boolean).join(' · ');
};
