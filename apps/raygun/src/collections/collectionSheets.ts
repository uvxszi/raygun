import { create } from 'zustand';

type CollectionSheetsState = {
  /** Import songs is open (M46.45: from Library → Audio, its menu, or a playlist). */
  importing: boolean;
  /** Merge playlists is open, with these picked first. */
  merging: string[] | null;
  openImport: () => void;
  openMerge: (picked?: string[]) => void;
  close: () => void;
};

/** Sheets about playlists that open from anywhere: menus, buttons, the library (M46.45). */
export const useCollectionSheets = create<CollectionSheetsState>((set) => ({
  importing: false,
  merging: null,
  openImport: () => set({ importing: true, merging: null }),
  openMerge: (picked = []) => set({ merging: picked, importing: false }),
  close: () => set({ importing: false, merging: null }),
}));
