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

/** The artists of a song or an album to pick one of (Go to artists). */
export type ArtistsToPick = {
  names: string[];
  /** Where the menu was: on wide windows the tiles pop up there. */
  anchor?: HTMLElement | null;
};

type MenuSheetsState = {
  adding: SongsToAdd | null;
  editing: DetailsToEdit | null;
  exporting: TitleToExport | null;
  artists: ArtistsToPick | null;
  addToPlaylist: (songs: SongsToAdd) => void;
  editDetails: (details: DetailsToEdit) => void;
  exportTitle: (title: TitleToExport) => void;
  pickArtist: (artists: ArtistsToPick) => void;
  close: () => void;
};

const NONE = { adding: null, editing: null, exporting: null, artists: null };

/**
 * The sheets cards' menus open: Add to playlist for many songs, Edit details, Export, and the
 * artists to go to.
 */
export const useMenuSheetsStore = create<MenuSheetsState>((set) => ({
  ...NONE,
  addToPlaylist: (adding) => set({ ...NONE, adding }),
  editDetails: (editing) => set({ ...NONE, editing }),
  exportTitle: (exporting) => set({ ...NONE, exporting }),
  pickArtist: (artists) => set({ ...NONE, artists }),
  close: () => set(NONE),
}));
