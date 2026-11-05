import type { LucideIcon } from 'lucide-react';
import type { MouseEvent as ReactMouseEvent } from 'react';
import { create } from 'zustand';

/** One line of an item's ⋮ menu (M46.8). */
export type MenuItem =
  | {
      kind?: 'item';
      label: string;
      icon?: LucideIcon;
      /** A key hint shown on the right, e.g. "Ctrl+C". */
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      /** Asked first, in the menu's place (M46.45: "Delete “Road trip”?"). */
      confirm?: string;
      onSelect: () => void;
    }
  | { kind: 'separator' }
  /** What the menu is for; a phone's menu sheet shows it with its cover and the line under it. */
  | { kind: 'heading'; label: string; subtitle?: string | null; artworkUrl?: string | null };

type ContextMenuState = {
  open: { x: number; y: number; items: MenuItem[] } | null;
  show: (x: number, y: number, items: MenuItem[]) => void;
  close: () => void;
};

export const useContextMenuStore = create<ContextMenuState>((set) => ({
  open: null,
  show: (x, y, items) => set({ open: { x, y, items } }),
  close: () => set({ open: null }),
}));

/**
 * Opens `items` at the pointer from `onContextMenu` (a right click, a long press on phones).
 * Only the quick access panel's pins use it (`RailPins`); everything else has its ⋮.
 */
export const openContextMenu = (event: ReactMouseEvent | MouseEvent, items: MenuItem[]) => {
  event.preventDefault();
  useContextMenuStore.getState().show(event.clientX, event.clientY, items);
};

/** Opens an item's menu from its ⋮ button: under it (a phone shows a sheet). */
export const openMenuAt = (anchor: HTMLElement, items: MenuItem[]) => {
  const box = anchor.getBoundingClientRect();
  useContextMenuStore.getState().show(box.left, box.bottom, items);
};
