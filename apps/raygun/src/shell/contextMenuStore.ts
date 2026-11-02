import type { LucideIcon } from 'lucide-react';
import type { MouseEvent as ReactMouseEvent } from 'react';
import { create } from 'zustand';

/** One line of a right-click menu (M46.8). */
export type MenuItem =
  | {
      kind?: 'item';
      label: string;
      icon?: LucideIcon;
      /** A key hint shown on the right, e.g. "Ctrl+C". */
      hint?: string;
      danger?: boolean;
      disabled?: boolean;
      onSelect: () => void;
    }
  | { kind: 'separator' }
  | { kind: 'heading'; label: string };

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
 * Opens Raygun's right-click menu with `items` at the pointer (M46.8). Components call it from
 * `onContextMenu`; the shell then adds what applies to the spot (a link, selected text, the page).
 */
export const openContextMenu = (event: ReactMouseEvent | MouseEvent, items: MenuItem[]) => {
  event.preventDefault();
  const native = 'nativeEvent' in event ? event.nativeEvent : event;
  // The shell's menu reads them from the native event; the innermost component's come first.
  const carried = native as unknown as { raygunMenu?: MenuItem[] };
  carried.raygunMenu ??= items;
};

/** Items a component put on the event, if any. */
export const menuFromEvent = (event: MouseEvent): MenuItem[] | undefined =>
  (event as unknown as { raygunMenu?: MenuItem[] }).raygunMenu;
