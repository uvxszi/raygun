import { create } from 'zustand';

import { useIsDesktop } from './useIsDesktop';
import { useSideRail } from './useWideScreen';

/** Whether the phone's quick access drawer (the sidebar's pins) is open. */
type QuickAccessState = {
  open: boolean;
  setOpen: (open: boolean) => void;
};

export const useQuickAccessStore = create<QuickAccessState>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
}));

/** Quick access needs the drawer where there's no sidebar: phones and narrow tablets. */
export const useQuickAccessShown = () => {
  const desktop = useIsDesktop();
  const sideRail = useSideRail();
  return !desktop && !sideRail;
};
