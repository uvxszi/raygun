import type { TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

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

type MenuSheetsState = {
  adding: SongsToAdd | null;
  editing: DetailsToEdit | null;
  addToPlaylist: (songs: SongsToAdd) => void;
  editDetails: (details: DetailsToEdit) => void;
  close: () => void;
};

/** The sheets cards' menus open: Add to playlist for many songs, and Edit details. */
export const useMenuSheetsStore = create<MenuSheetsState>((set) => ({
  adding: null,
  editing: null,
  addToPlaylist: (adding) => set({ adding, editing: null }),
  editDetails: (editing) => set({ editing, adding: null }),
  close: () => set({ adding: null, editing: null }),
}));
