import { create } from 'zustand';

type CollectionSheetsState = {
  /** Import songs is open (M46.45: from Library → Audio, its menu, or a playlist). */
  importing: boolean;
  /** The link Import songs opens with (a Spotify link shared to Raygun); empty otherwise. */
  link: string;
  /** Merge playlists is open, with these picked first. */
  merging: string[] | null;
  openImport: () => void;
  openImportLink: (link: string) => void;
  openMerge: (picked?: string[]) => void;
  close: () => void;
};

/** Sheets about playlists that open from anywhere: menus, buttons, the library (M46.45). */
export const useCollectionSheets = create<CollectionSheetsState>((set) => ({
  importing: false,
  link: '',
  merging: null,
  openImport: () => set({ importing: true, link: '', merging: null }),
  openImportLink: (link) => set({ importing: true, link, merging: null }),
  openMerge: (picked = []) => set({ merging: picked, importing: false }),
  close: () => set({ importing: false, link: '', merging: null }),
}));
