import type { TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

type SourcePickerState = {
  song: TrackInput | null;
  open: (song: TrackInput) => void;
  close: () => void;
};

/** Opens a song's sources from anywhere: song menus, unavailable rows (M46.31). */
export const useSourcePickerStore = create<SourcePickerState>((set) => ({
  song: null,
  open: (song) => set({ song }),
  close: () => set({ song: null }),
}));
