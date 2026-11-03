import { create } from 'zustand';

/** A cover to show large and save (M46.26). */
export type CoverToSave = {
  url: string;
  /** Names the saved file ("Artist - Album"). */
  title: string;
};

type CoverSheetState = {
  cover: CoverToSave | null;
  open: (cover: CoverToSave) => void;
  close: () => void;
};

/** Opens a cover's sheet from anywhere: the player's artwork, album pages, song menus. */
export const useCoverSheetStore = create<CoverSheetState>((set) => ({
  cover: null,
  open: (cover) => set({ cover }),
  close: () => set({ cover: null }),
}));

/** The file name a song's cover gets: its artist and album, else its title. */
export const coverTitle = (song: { title: string; artist: string | null; album: string | null }) =>
  [song.artist, song.album ?? song.title].filter(Boolean).join(' - ');
