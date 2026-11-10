import type { SongDownload } from '@raygun/bindings';
import { create } from 'zustand';

import { isBusy, shareOf } from './songDownloadState';

/** Where the dock's minimized state lives: for the session, so a new start shows the bar again. */
const MINIMIZED_KEY = 'raygun.downloadDock.minimized';

const readMinimized = () => {
  try {
    return sessionStorage.getItem(MINIMIZED_KEY) === '1';
  } catch {
    return false;
  }
};

type DownloadDockState = {
  /** The recording bar put away into the round button, until it's tapped. */
  minimized: boolean;
  /** When songs started being on their way (Unix ms), for the round button's progress. */
  since: number | null;
  setMinimized: (minimized: boolean) => void;
  /** Follows whether songs are on their way: a new batch starts when the first one comes. */
  follow: (busy: boolean, now: number) => void;
};

/**
 * The dock above the player bar: the recording bar, or the round button it's minimized into,
 * which also counts the songs downloading.
 */
export const useDownloadDockStore = create<DownloadDockState>((set) => ({
  minimized: readMinimized(),
  since: null,
  setMinimized: (minimized) => {
    try {
      if (minimized) sessionStorage.setItem(MINIMIZED_KEY, '1');
      else sessionStorage.removeItem(MINIMIZED_KEY);
    } catch {
      // Without storage it's remembered until the page reloads.
    }
    set({ minimized });
  },
  follow: (busy, now) =>
    set((state) =>
      busy
        ? state.since === null
          ? { since: now }
          : state
        : state.since === null
          ? state
          : { since: null },
    ),
}));

/** What the dock shows of the songs on their way. */
export type DockSummary = {
  /** Songs left: queued or downloading (recordings and captures too). */
  left: number;
  /** Of those, songs to record (the recording bar's). */
  recordings: number;
  /**
   * How far the batch is, 0–1: songs finished since it started count whole, the ones downloading
   * by their share, queued ones not yet. Finished songs leaving the count don't send it back.
   */
  progress: number;
};

export const IDLE_DOCK: DockSummary = { left: 0, recordings: 0, progress: 0 };

/**
 * The dock's summary of the downloads: `since` is when the batch started (Unix ms); songs that
 * finished or stopped since then still count toward its progress.
 */
export const dockSummary = (list: readonly SongDownload[], since: number | null): DockSummary => {
  const busy = list.filter(isBusy);
  if (busy.length === 0) return IDLE_DOCK;
  const finished =
    since === null
      ? 0
      : list.filter((item) => !isBusy(item) && item.finishedAt !== null && item.finishedAt >= since)
          .length;
  const moving = busy.reduce((sum, item) => sum + (shareOf(item) ?? 0), 0);
  return {
    left: busy.length,
    recordings: busy.filter((item) => item.kind === 'recording').length,
    // Rounded to a percent, so a report that moves less doesn't re-render the button.
    progress: Math.round(((finished + moving) / (finished + busy.length)) * 100) / 100,
  };
};
