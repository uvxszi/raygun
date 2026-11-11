import type { VideoAspect } from '@raygun/bindings';
import { useEffect, useMemo, useState } from 'react';
import type { RefObject } from 'react';

/** The part of the player the picture covers, in its pixels, with the player's own size. */
export type PictureBox = {
  left: number;
  top: number;
  width: number;
  height: number;
  stageWidth: number;
  stageHeight: number;
};

/**
 * Where a `pictureWidth`×`pictureHeight` picture shows on a `stageWidth`×`stageHeight` player
 * with `aspect`: fitted (letterboxed) for `fit`, the whole player for `crop` (the rest is cut
 * off) and `stretch`. Without a picture yet, a 16:9 one is assumed, so subtitles are never sized
 * to a tall window's height (`subtitleArea` in the Android player).
 */
export const pictureBox = (
  stageWidth: number,
  stageHeight: number,
  pictureWidth: number,
  pictureHeight: number,
  aspect: VideoAspect,
): PictureBox => {
  const known = pictureWidth > 0 && pictureHeight > 0;
  if (stageWidth <= 0 || stageHeight <= 0 || (known && aspect !== 'fit')) {
    return {
      left: 0,
      top: 0,
      width: Math.max(0, stageWidth),
      height: Math.max(0, stageHeight),
      stageWidth,
      stageHeight,
    };
  }
  const ratio = known ? pictureWidth / pictureHeight : 16 / 9;
  let width = stageWidth;
  let height = stageWidth / ratio;
  if (height > stageHeight) {
    height = stageHeight;
    width = Math.min(stageWidth, stageHeight * ratio);
  }
  return {
    left: (stageWidth - width) / 2,
    top: (stageHeight - height) / 2,
    width,
    height,
    stageWidth,
    stageHeight,
  };
};

/** Subtitle lines' size at 100 %: a share of the picture's height as it shows. */
const PICTURE_SHARE = 0.046;
/** Below this, lines can't be read (a tiny window). */
const MIN_FONT_PX = 10;

/**
 * Subtitle lines' size in pixels: a share of the picture's height as it shows (not the window's,
 * so a tall window or a letterboxed film doesn't make them too large), times the size setting.
 */
export const subtitleFontPx = (box: PictureBox, size: number) =>
  Math.max(MIN_FONT_PX, box.height * PICTURE_SHARE * (size / 100));

type Sizes = { stage: [number, number]; picture: [number, number] };

/**
 * Where the `<video>`'s picture shows (it fills the player), followed as the window, the video
 * or its size change; null until the element has a size.
 */
export const usePictureBox = (
  videoRef: RefObject<HTMLVideoElement | null>,
  aspect: VideoAspect,
): PictureBox | null => {
  const [sizes, setSizes] = useState<Sizes>({ stage: [0, 0], picture: [0, 0] });
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const measure = () =>
      setSizes((previous) => {
        const next: Sizes = {
          stage: [video.clientWidth, video.clientHeight],
          picture: [video.videoWidth, video.videoHeight],
        };
        const same =
          previous.stage[0] === next.stage[0] &&
          previous.stage[1] === next.stage[1] &&
          previous.picture[0] === next.picture[0] &&
          previous.picture[1] === next.picture[1];
        return same ? previous : next;
      });
    measure();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(video);
    // `resize` is the video's: its picture size changed (another quality, another stream).
    video.addEventListener('loadedmetadata', measure);
    video.addEventListener('resize', measure);
    window.addEventListener('resize', measure);
    return () => {
      observer?.disconnect();
      video.removeEventListener('loadedmetadata', measure);
      video.removeEventListener('resize', measure);
      window.removeEventListener('resize', measure);
    };
  }, [videoRef]);
  return useMemo(
    () =>
      sizes.stage[0] > 0 && sizes.stage[1] > 0
        ? pictureBox(sizes.stage[0], sizes.stage[1], sizes.picture[0], sizes.picture[1], aspect)
        : null,
    [sizes, aspect],
  );
};
