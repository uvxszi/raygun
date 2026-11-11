import type { SubtitleStyle } from '@raygun/bindings';
import type { CSSProperties } from 'react';

import type { PictureBox } from './pictureBox';

/** The main subtitles at the picture's foot, or the second ones at its top. */
export type SubtitlePlace = 'bottom' | 'top';

/** How far up the control bar reaches over the player's foot; lifted lines stay above it. */
const BAR_PX = 110;
/** Where the top bar's buttons end; the second subtitles stay below them while it shows. */
const TOP_BAR_PX = 64;
/** Where the second subtitles start, as a share of the picture's height from its top. */
const TOP_SHARE = 0.06;

/**
 * Where the lines go: over the picture's own part of the player, at the chosen height in it, as
 * wide as the chosen share of its width (longer lines wrap). Before the player has a size, the
 * lines take the window's width and are lifted by their place, not slid.
 */
export const subtitlePlacement = (
  box: PictureBox | null | undefined,
  style: Required<SubtitleStyle>,
  lifted: boolean,
  at: SubtitlePlace,
): CSSProperties => {
  const margin = (100 - style.maxWidth) / 2;
  if (!box || box.height <= 0) {
    const sides = { left: 0, right: 0, paddingLeft: `${margin}%`, paddingRight: `${margin}%` };
    return at === 'top'
      ? { ...sides, top: `${TOP_SHARE * 100}%` }
      : {
          ...sides,
          bottom: lifted ? `max(${style.position}%, ${BAR_PX}px)` : `${style.position}%`,
        };
  }
  const inset = (box.width * margin) / 100;
  const sides = { left: box.left, width: box.width, paddingLeft: inset, paddingRight: inset };
  if (at === 'top') return { ...sides, top: box.top + box.height * TOP_SHARE };
  const foot = box.stageHeight - (box.top + box.height);
  return { ...sides, bottom: foot + (box.height * style.position) / 100 };
};

/**
 * How far the lines slide while the controls show, in px: up (negative) until they clear the
 * bottom bar, or for the second subtitles down until they clear the top bar.
 */
export const subtitleSlide = (
  box: PictureBox | null | undefined,
  style: Required<SubtitleStyle>,
  lifted: boolean,
  at: SubtitlePlace,
): number => {
  if (!lifted || !box || box.height <= 0) return 0;
  if (at === 'top') return Math.max(0, TOP_BAR_PX - (box.top + box.height * TOP_SHARE));
  const foot = box.stageHeight - (box.top + box.height);
  const rise = Math.max(0, BAR_PX - (foot + (box.height * style.position) / 100));
  return rise > 0 ? -rise : 0;
};
