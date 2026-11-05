import type { TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

import type { TrackMenuExtras } from '../music/useTrackMenu';

type OpenOptions = {
  /** Set when opened from a playlist page, which adds "Remove from this playlist". */
  playlistItemId?: string;
  /** The button pressed: on wide windows the actions pop up beside it (M46.7). */
  anchor?: HTMLElement | null;
  /** What the row adds to the song's menu (play from here, Not interested…). */
  extras?: TrackMenuExtras;
};

type TrackActionsState = {
  /** The track the actions are open for. */
  track: TrackInput | null;
  playlistItemId: string | null;
  anchor: HTMLElement | null;
  extras: TrackMenuExtras;
  /** Opens the actions; the same button pressed again closes them. */
  open: (track: TrackInput, options?: OpenOptions) => void;
  close: () => void;
};

export const useTrackActionsStore = create<TrackActionsState>((set, get) => ({
  track: null,
  playlistItemId: null,
  anchor: null,
  extras: {},
  open: (track, options) => {
    const anchor = options?.anchor ?? null;
    if (anchor && get().track && get().anchor === anchor) {
      set({ track: null, playlistItemId: null, anchor: null, extras: {} });
      return;
    }
    set({
      track,
      playlistItemId: options?.playlistItemId ?? null,
      anchor,
      extras: options?.extras ?? {},
    });
  },
  close: () => set({ track: null, playlistItemId: null, anchor: null, extras: {} }),
}));
