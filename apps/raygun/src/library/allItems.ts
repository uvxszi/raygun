import type {
  AudiobookSummary,
  MangaLibraryEntry,
  PlaylistSummary,
  PodcastSummary,
  Website,
} from '@raygun/bindings';

import type { Medium } from '../manga/medium';
import { cardPage } from '../manga/titleMenu';
import { playlistPin } from '../shell/railPrefs';
import type { PinTarget } from '../shell/railPrefs';
import { websitePin } from '../websites/websitePin';

/** Something Library → All shows, which can be picked and has a menu. */
export type AllItem =
  | { kind: 'playlist'; playlist: PlaylistSummary }
  | { kind: 'podcast'; show: PodcastSummary }
  | { kind: 'audiobook'; book: AudiobookSummary }
  | { kind: 'title'; medium: Medium; entry: MangaLibraryEntry }
  | { kind: 'website'; site: Website };

/** What an item is, for the actions a selection shares: a title's medium stands for it. */
export type AllKind = Exclude<AllItem['kind'], 'title'> | Medium;

export const allKind = (item: AllItem): AllKind =>
  item.kind === 'title' ? item.medium : item.kind;

/** An item's key in the selection: its kind and id. */
export const allItemKey = (item: AllItem): string => {
  switch (item.kind) {
    case 'playlist':
      return `playlist:${item.playlist.id}`;
    case 'podcast':
      return `podcast:${item.show.id}`;
    case 'audiobook':
      return `audiobook:${item.book.id}`;
    case 'title':
      return `${item.medium}:${item.entry.titleId}`;
    case 'website':
      return `website:${item.site.id}`;
  }
};

export const allItemName = (item: AllItem): string => {
  switch (item.kind) {
    case 'playlist':
      return item.playlist.name;
    case 'podcast':
      return item.show.title;
    case 'audiobook':
      return item.book.title;
    case 'title':
      return item.entry.title;
    case 'website':
      return item.site.name;
  }
};

/** The item's page as quick access pins it. */
export const allItemPin = (item: AllItem): PinTarget => {
  switch (item.kind) {
    case 'playlist':
      return playlistPin(item.playlist);
    case 'podcast':
      return {
        kind: 'podcast',
        to: '/podcast',
        search: { id: item.show.id },
        name: item.show.title,
        artworkUrl: item.show.artworkUrl,
      };
    case 'audiobook':
      return {
        kind: 'audiobook',
        to: '/audiobook',
        search: { id: item.book.id },
        name: item.book.title,
        artworkUrl: item.book.artworkUrl,
      };
    case 'title': {
      const page = cardPage({
        medium: item.medium,
        sourceId: item.entry.sourceId,
        id: item.entry.remoteId,
        title: item.entry.title,
        coverUrl: item.entry.coverUrl,
      });
      return {
        kind: item.medium,
        to: page.to,
        search: page.search,
        name: item.entry.title,
        artworkUrl: item.entry.coverUrl,
      };
    }
    case 'website':
      return websitePin(item.site);
  }
};

/** Marking as done and not done, in the words of the kinds picked (i18n keys). */
export type DoneWords = { done: string; notDone: string };

/** What every picked item can have done to it; Pin and Remove from library always can. */
export type AllActions = {
  /** Playlists, shows and books: their songs, episodes and chapters played next or last. */
  queue: boolean;
  /** Comics, books, videos, shows and audiobooks: read, watched, played or finished. */
  done: DoneWords | null;
  /** Playlists, shows, audiobooks, and comics, books and videos from sources. */
  download: boolean;
  /** Comics, books and videos: the library's favorites. */
  favorite: boolean;
  /** Playlists alone: their songs added to another, or merged together (two or more). */
  playlists: boolean;
};

const READING: AllKind[] = ['manga', 'novel'];
const QUEUED: AllKind[] = ['playlist', 'podcast', 'audiobook'];
const DONE: AllKind[] = ['manga', 'novel', 'video', 'podcast', 'audiobook'];

const doneWords = (kinds: AllKind[]): DoneWords | null => {
  if (!kinds.every((kind) => DONE.includes(kind))) return null;
  if (kinds.every((kind) => READING.includes(kind))) {
    return { done: 'library.select.markRead', notDone: 'library.select.markUnread' };
  }
  if (kinds.every((kind) => kind === 'video')) {
    return { done: 'library.select.markWatched', notDone: 'library.select.markUnwatched' };
  }
  if (kinds.every((kind) => kind === 'podcast')) {
    return { done: 'podcasts.markPlayed', notDone: 'podcasts.markUnplayed' };
  }
  if (kinds.every((kind) => kind === 'audiobook')) {
    return { done: 'audiobooks.markFinished', notDone: 'audiobooks.markUnfinished' };
  }
  return { done: 'library.all.markDone', notDone: 'library.all.markNotDone' };
};

/** The actions the picked items share (a mix gets what its kinds share). */
export const sharedActions = (items: AllItem[]): AllActions => {
  const kinds = [...new Set(items.map(allKind))];
  const titles = items.every((item) => item.kind === 'title');
  return {
    queue: kinds.every((kind) => QUEUED.includes(kind)),
    done: doneWords(kinds),
    download: items.every(
      (item) =>
        item.kind === 'playlist' ||
        item.kind === 'podcast' ||
        item.kind === 'audiobook' ||
        (item.kind === 'title' && !item.entry.local),
    ),
    favorite: titles,
    playlists: kinds.length === 1 && kinds[0] === 'playlist',
  };
};
