import type { TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

import type { Medium } from '../manga/medium';

/** Songs to add to a playlist from a card's menu: an album's, a mix's, a playlist's. */
export type SongsToAdd = {
  /** What they are ("Random Access Memories"). */
  title: string;
  /** Fetches the songs when a playlist is picked (an album's are asked for only then). */
  load: () => Promise<TrackInput[]>;
};

/** An album or an artist to edit the details of (M46.30), from a card's menu. */
export type DetailsToEdit =
  | { kind: 'album'; title: string; artist: string | null; artworkUrl: string | null }
  | { kind: 'artist'; name: string; artworkUrl: string | null };

/** A stored comic, book or video to export, from a card's menu. */
export type TitleToExport = { medium: Medium; titleId: string };

type MenuSheetsState = {
  adding: SongsToAdd | null;
  editing: DetailsToEdit | null;
  exporting: TitleToExport | null;
  addToPlaylist: (songs: SongsToAdd) => void;
  editDetails: (details: DetailsToEdit) => void;
  exportTitle: (title: TitleToExport) => void;
  close: () => void;
};

/** The sheets cards' menus open: Add to playlist for many songs, Edit details, Export. */
export const useMenuSheetsStore = create<MenuSheetsState>((set) => ({
  adding: null,
  editing: null,
  exporting: null,
  addToPlaylist: (adding) => set({ adding, editing: null, exporting: null }),
  editDetails: (editing) => set({ editing, adding: null, exporting: null }),
  exportTitle: (exporting) => set({ exporting, adding: null, editing: null }),
  close: () => set({ adding: null, editing: null, exporting: null }),
}));
