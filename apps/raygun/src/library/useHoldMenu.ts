import { usePickPress } from '../select/usePickPress';
import type { PickState } from '../select/usePickPress';
import { openMenuAtPoint } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';

/** Nothing to pick: Favorites, Downloaded and Local files are blocks, not library items. */
const NOT_PICKED: PickState = {
  selected: false,
  selecting: false,
  toggle: () => undefined,
  extend: () => undefined,
};

/**
 * Handlers that open `items` where a finger holds the element or a mouse right-clicks it (Library
 * → All's blocks); its ⋮ and buttons keep their own presses, and the webview's own menu and
 * dragging never start.
 */
export const useHoldMenu = (items: () => MenuItem[]) =>
  usePickPress(NOT_PICKED, (at) => openMenuAtPoint(at, items()));
