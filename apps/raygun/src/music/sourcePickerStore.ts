import type { SongDownload, TrackInput } from '@raygun/bindings';
import { create } from 'zustand';

type SourcePickerState = {
  song: TrackInput | null;
  /** The download a pick goes to (M46.47): its source is set and it starts again. */
  download: string | null;
  open: (song: TrackInput, download?: string) => void;
  close: () => void;
};

/** Opens a song's sources from anywhere: song menus, unavailable rows (M46.31), downloads. */
export const useSourcePickerStore = create<SourcePickerState>((set) => ({
  song: null,
  download: null,
  open: (song, download) => set({ song, download: download ?? null }),
  close: () => set({ song: null, download: null }),
}));

/** Choose a source for a failed or YouTube-only download (M46.47). */
export const chooseDownloadSource = (item: SongDownload) =>
  useSourcePickerStore.getState().open(
    {
      title: item.title,
      artist: item.artist,
      album: item.album,
      durationMs: item.durationMs,
      artworkUrl: null,
      source: null,
    },
    item.id,
  );
