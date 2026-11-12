/**
 * How subtitles look in the desktop player, its Style card and Settings' preview: their fonts
 * and edges, as Settings → Video player → Subtitles sets them.
 */

import type { SubtitleEdge, SubtitleFont } from '@raygun/bindings';
import type { CSSProperties } from 'react';

import { INTERFACE_STACK, quoteFamily, useInstalledFamily } from '../fonts/systemFonts';
import { useSettings } from '../settings/useSettings';

/**
 * Each subtitle font as CSS (`SubtitleFont` in raygun-model): Android's family name first, which
 * its webview knows (`casual` is Coming Soon there), then the nearest fonts on Windows, macOS and
 * Linux, then a generic family. The phone's player draws the same families natively
 * (`PlayerTrackNames.kt`). Default is the interface's own font.
 */
export const SUBTITLE_FONT_STACKS: Record<SubtitleFont, string> = {
  default: 'inherit',
  sans: 'ui-sans-serif, system-ui, sans-serif',
  condensed:
    '"sans-serif-condensed", "Roboto Condensed", "Arial Narrow", Bahnschrift, ui-sans-serif, sans-serif',
  mono: 'ui-monospace, Consolas, "Cascadia Mono", "Droid Sans Mono", monospace',
  serif: 'ui-serif, Georgia, "Noto Serif", serif',
  serifMono: '"serif-monospace", "Courier New", Courier, "Cutive Mono", monospace',
  casual: 'casual, "Comic Sans MS", "Comic Neue", "Coming Soon", cursive',
  cursive: '"Segoe Script", "Dancing Script", "Brush Script MT", cursive',
  smallCaps: '"sans-serif-smallcaps", "Carrois Gothic SC", ui-sans-serif, sans-serif',
};

/**
 * How subtitles in `font` are drawn: its family stack, narrowed for Condensed and in small
 * capitals for Small capitals. A font of the device's (`family`) goes in front of the stack and
 * is drawn as it is. An unknown font (from a newer version) draws as the default.
 */
export const subtitleFontCss = (font: SubtitleFont, family?: string | null): CSSProperties => {
  const stack = SUBTITLE_FONT_STACKS[font] ?? 'inherit';
  if (family) {
    return {
      fontFamily: `${quoteFamily(family)}, ${stack === 'inherit' ? INTERFACE_STACK : stack}`,
    };
  }
  return {
    fontFamily: stack,
    ...(font === 'condensed' && { fontStretch: 'condensed' }),
    ...(font === 'smallCaps' && { fontVariantCaps: 'small-caps' }),
  };
};

/**
 * The edge around subtitle text as a `text-shadow`, `px` thick: outlines over a video (2) get a
 * soft glow besides, as the player draws them; small samples take 1.
 */
export const edgeShadow = (edge: SubtitleEdge, color: string, px: number): string => {
  switch (edge) {
    case 'outline': {
      const sides = [
        `-${px}px -${px}px`,
        `${px}px -${px}px`,
        `-${px}px ${px}px`,
        `${px}px ${px}px`,
      ];
      const outline = sides.map((offset) => `${offset} 0 ${color}`);
      return [...outline, ...(px > 1 ? [`0 0 ${px + 1}px ${color}`] : [])].join(', ');
    }
    case 'shadow':
      return `${px + 1}px ${px + 1}px ${px + 2}px ${color}`;
    case 'raised':
      return `-1px -1px 0 ${color}`;
    case 'depressed':
      return `1px 1px 0 ${color}`;
    default:
      return 'none';
  }
};

/**
 * The device's font picked for subtitles (`fonts.subtitles`, kept to this device), as this
 * device draws it; null for none or one it doesn't have.
 */
export const useSubtitleFamily = (): string | null =>
  useInstalledFamily(useSettings().data?.fonts?.subtitles);
