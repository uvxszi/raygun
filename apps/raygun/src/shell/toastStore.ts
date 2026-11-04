import { create } from 'zustand';

/** A short note at the bottom of the window, with Undo when the change can be taken back. */
export type Toast = {
  id: number;
  message: string;
  /** Takes the change back; the toast then goes. */
  undo?: () => void;
  /** The height of the player bar and tabs under it when it came, px. */
  bottom: number;
};

/** The space taken at the bottom by the player bar and the tabs, so the toast sits above them. */
const bottomBars = () => {
  const tops = ['mini-player', 'bottom-tabs']
    .map((id) => document.querySelector(`[data-testid="${id}"]`)?.getBoundingClientRect().top)
    .filter((top): top is number => top !== undefined && top > 0);
  return tops.length > 0 ? Math.max(0, window.innerHeight - Math.min(...tops)) : 0;
};

type ToastState = {
  toast: Toast | null;
  show: (message: string, undo?: () => void) => void;
  dismiss: (id?: number) => void;
};

let next = 1;

export const useToastStore = create<ToastState>((set, get) => ({
  toast: null,
  show: (message, undo) => set({ toast: { id: next++, message, undo, bottom: bottomBars() } }),
  // With an id, only that toast goes (a later one stays).
  dismiss: (id) => {
    if (id === undefined || get().toast?.id === id) set({ toast: null });
  },
}));

/** Shows `message` for a few seconds, with Undo when `undo` is given. */
export const showToast = (message: string, undo?: () => void) =>
  useToastStore.getState().show(message, undo);
