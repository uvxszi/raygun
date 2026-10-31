import { create } from 'zustand';

type PlayerUiState = {
  sheetOpen: boolean;
  openSheet: () => void;
  closeSheet: () => void;
};

export const usePlayerUiStore = create<PlayerUiState>((set) => ({
  sheetOpen: false,
  openSheet: () => set({ sheetOpen: true }),
  closeSheet: () => set({ sheetOpen: false }),
}));
