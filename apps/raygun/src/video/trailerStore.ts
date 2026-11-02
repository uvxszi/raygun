import { events } from '@raygun/bindings';
import type { YouTubeVideo } from '@raygun/bindings';
import { useEffect } from 'react';
import { create } from 'zustand';

type TrailerState = {
  video: YouTubeVideo | null;
  open: (video: YouTubeVideo) => void;
  close: () => void;
};

/** The YouTube video playing in its own player (trailers, M46.4). */
export const useTrailerStore = create<TrailerState>((set) => ({
  video: null,
  open: (video) => set({ video }),
  close: () => set({ video: null }),
}));

/** Opens the trailer player when Rust says so. Mount once, in the app shell. */
export const useYouTubeVideoEvents = () => {
  useEffect(() => {
    const unlisten = events.youTubeVideoEvent.listen(({ payload }) =>
      useTrailerStore.getState().open(payload),
    );
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);
};
