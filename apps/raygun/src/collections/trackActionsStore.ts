import type { TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

type TrackActionsState = {
  /** The track the action sheet is open for. */
  track: TrackInput | null;
  /** Set when opened from a playlist page, which adds "Remove from this playlist". */
  playlistItemId: string | null;
  open: (track: TrackInput, options?: { playlistItemId?: string }) => void;
  close: () => void;
};

export const useTrackActionsStore = create<TrackActionsState>((set) => ({
  track: null,
  playlistItemId: null,
  open: (track, options) => set({ track, playlistItemId: options?.playlistItemId ?? null }),
  close: () => set({ track: null, playlistItemId: null }),
}));
