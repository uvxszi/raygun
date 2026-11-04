import { commands } from '@raygun/bindings';
import { create } from 'zustand';

/** The side panel beside the page on wide windows (M46.4, M46.7): what plays, or the queue. */
export type PlayerPanel = 'nowPlaying' | 'queue';

const PANEL_KEY = 'raygun.playerPanel';

/** The panel last left open; Now playing at first, as Spotify does. */
const savedPanel = (): PlayerPanel | null => {
  try {
    const saved = localStorage.getItem(PANEL_KEY);
    if (saved === 'none') return null;
    return saved === 'queue' ? 'queue' : 'nowPlaying';
  } catch {
    return 'nowPlaying';
  }
};

const savePanel = (panel: PlayerPanel | null) => {
  try {
    localStorage.setItem(PANEL_KEY, panel ?? 'none');
  } catch {
    // A convenience only: without storage the panel opens at its default next time.
  }
};

/**
 * What the full player's main area shows (M46.7): the artwork, the lyrics or what's next. Side by
 * side, the artwork stays on the left and this picks the right column (lyrics for `artwork`).
 */
export type SheetView = 'artwork' | 'lyrics' | 'queue';

type PlayerUiState = {
  sheetOpen: boolean;
  sheetView: SheetView;
  panel: PlayerPanel | null;
  /** A YouTube song shows its video instead of its artwork: only once asked for (M46.8). */
  videoShown: boolean;
  /** The volume unmuting goes back to. */
  unmutedVolume: number;
  /** Lyrics can be selected and copied, toggled from their right-click menu. */
  lyricsSelectable: boolean;
  setLyricsSelectable: (selectable: boolean) => void;
  openSheet: (options?: { view?: SheetView }) => void;
  closeSheet: () => void;
  setSheetView: (view: SheetView) => void;
  /** Opens `panel`, or closes the side panel when `panel` is what it shows. */
  togglePanel: (panel: PlayerPanel) => void;
  setPanel: (panel: PlayerPanel | null) => void;
  setVideoShown: (shown: boolean) => void;
  setUnmutedVolume: (volume: number) => void;
};

export const usePlayerUiStore = create<PlayerUiState>((set) => ({
  sheetOpen: false,
  sheetView: 'artwork',
  panel: savedPanel(),
  videoShown: false,
  unmutedVolume: 1,
  lyricsSelectable: false,
  setLyricsSelectable: (lyricsSelectable) => set({ lyricsSelectable }),
  openSheet: (options) => {
    // A field left focused (a search) keeps the phone's keyboard up, which squeezes the player.
    const focused = document.activeElement;
    if (
      focused instanceof HTMLInputElement ||
      focused instanceof HTMLTextAreaElement ||
      (focused instanceof HTMLElement && focused.isContentEditable)
    ) {
      focused.blur();
    }
    set((state) => ({ sheetOpen: true, sheetView: options?.view ?? state.sheetView }));
  },
  closeSheet: () => set({ sheetOpen: false }),
  setSheetView: (sheetView) => set({ sheetView }),
  togglePanel: (panel) =>
    set((state) => {
      const next = state.panel === panel ? null : panel;
      savePanel(next);
      return { panel: next };
    }),
  setPanel: (panel) => {
    savePanel(panel);
    set({ panel });
  },
  setVideoShown: (videoShown) => {
    set({ videoShown });
    // YouTube songs play as their audio file; the embed, which has the video, while it shows.
    void Promise.resolve(commands.playbackSetVideo(videoShown)).catch(() => undefined);
  },
  setUnmutedVolume: (unmutedVolume) => set({ unmutedVolume }),
}));
