import type { OnlineTrack } from '@raygun/bindings';
import { create } from 'zustand';

import type { Medium } from '../manga/medium';
import { trackKey } from '../music/trackKey';

/** A comic, book or video from a source's results: it may never have been opened. */
export type PickedTitle = {
  kind: 'title';
  medium: Medium;
  sourceId: string;
  /** The source's id (Stremio titles: `<type>:<id>`). */
  remoteId: string;
  title: string;
};

/** A song from a music source's results. */
export type PickedSong = { kind: 'song'; track: OnlineTrack };

export type Picked = PickedTitle | PickedSong;

/** What a picked item is called in the selection: one per item, whichever list shows it. */
export const pickedKey = (picked: Picked) =>
  picked.kind === 'song'
    ? `song:${trackKey(picked.track.title, picked.track.artist)}`
    : `${picked.medium}:${picked.sourceId}:${picked.remoteId}`;

type SelectionState = {
  /** Picked items in the order they were picked. */
  items: Picked[];
  /** Their keys, for a card to ask whether it's picked without walking the list. */
  keys: ReadonlySet<string>;
  toggle: (picked: Picked) => void;
  clear: () => void;
};

/**
 * The selection of search and source results (M46.50): long press or Ctrl/Shift-click starts it,
 * taps then add and take out. Pages that show results host one bar for it (`SelectionScope`).
 */
export const useSelectionStore = create<SelectionState>((set) => ({
  items: [],
  keys: new Set(),
  toggle: (picked) =>
    set((state) => {
      const key = pickedKey(picked);
      const items = state.keys.has(key)
        ? state.items.filter((item) => pickedKey(item) !== key)
        : [...state.items, picked];
      return { items, keys: new Set(items.map(pickedKey)) };
    }),
  clear: () => set({ items: [], keys: new Set() }),
}));

/** What the picked items are, for the actions they share. */
export type PickedKinds = {
  songs: PickedSong[];
  titles: PickedTitle[];
  /** The one medium of the titles, when they're all the same. */
  medium: Medium | null;
};

export const pickedKinds = (items: Picked[]): PickedKinds => {
  const songs = items.filter((item): item is PickedSong => item.kind === 'song');
  const titles = items.filter((item): item is PickedTitle => item.kind === 'title');
  const media = new Set(titles.map((title) => title.medium));
  return { songs, titles, medium: media.size === 1 ? ([...media][0] ?? null) : null };
};
