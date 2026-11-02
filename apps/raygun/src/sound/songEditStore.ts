import type { TrackQuery } from '@raygun/bindings';
import { create } from 'zustand';

export type SongEditTab = 'sound' | 'lyrics';

type SongEditState = {
  /** The song being edited (M46.12). */
  song: TrackQuery | null;
  tab: SongEditTab;
  /** The button pressed: on wide windows the editor pops up beside it. */
  anchor: HTMLElement | null;
  open: (song: TrackQuery, options?: { tab?: SongEditTab; anchor?: HTMLElement | null }) => void;
  close: () => void;
};

/** Opens a song's sound and lyrics editor from anywhere: song menus, the player, the side panel. */
export const useSongEditStore = create<SongEditState>((set) => ({
  song: null,
  tab: 'sound',
  anchor: null,
  open: (song, options) =>
    set({ song, tab: options?.tab ?? 'sound', anchor: options?.anchor ?? null }),
  close: () => set({ song: null, anchor: null }),
}));
