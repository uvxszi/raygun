import { create } from 'zustand';

type PlayerUiState = {
  sheetOpen: boolean;
  /** The full player shows lyrics instead of the artwork. */
  lyrics: boolean;
  /** The queue panel beside the player bar (wide windows, M46.4). */
  queueOpen: boolean;
  /** The volume unmuting goes back to. */
  unmutedVolume: number;
  openSheet: (options?: { lyrics?: boolean }) => void;
  closeSheet: () => void;
  setLyrics: (shown: boolean) => void;
  toggleQueue: () => void;
  closeQueue: () => void;
  setUnmutedVolume: (volume: number) => void;
};

export const usePlayerUiStore = create<PlayerUiState>((set) => ({
  sheetOpen: false,
  lyrics: false,
  queueOpen: false,
  unmutedVolume: 1,
  openSheet: (options) =>
    set((state) => ({
      sheetOpen: true,
      queueOpen: false,
      lyrics: options?.lyrics ?? state.lyrics,
    })),
  closeSheet: () => set({ sheetOpen: false }),
  setLyrics: (lyrics) => set({ lyrics }),
  toggleQueue: () => set((state) => ({ queueOpen: !state.queueOpen })),
  closeQueue: () => set({ queueOpen: false }),
  setUnmutedVolume: (unmutedVolume) => set({ unmutedVolume }),
}));
