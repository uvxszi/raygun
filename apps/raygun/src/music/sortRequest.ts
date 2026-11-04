import { create } from 'zustand';

import type { TrackTableKind } from './tableView';

/**
 * A menu's Sort (Favorites, Downloaded): the next table of that kind to show opens its Sort by
 * popover once.
 */
type SortRequestState = {
  kind: TrackTableKind | null;
  request: (kind: TrackTableKind) => void;
  clear: () => void;
};

export const useSortRequest = create<SortRequestState>((set) => ({
  kind: null,
  request: (kind) => set({ kind }),
  clear: () => set({ kind: null }),
}));
