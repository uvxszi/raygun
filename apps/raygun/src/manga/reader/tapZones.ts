// The tap zone layouts are adapted from Mihon (Apache-2.0),
// app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/*.kt and ViewerNavigation.kt.

import type { ReaderMode, TapInvert, TapZones } from '@raygun/bindings';

/** What a tap does: show or hide the controls, or turn the page. */
export type TapAction = 'menu' | 'forward' | 'back';

/** Previous and next whatever the direction; left and right follow it (right to left books). */
type RegionKind = 'previous' | 'next' | 'left' | 'right';

/** A part of the screen, in fractions of its width and height. */
type Region = { left: number; top: number; right: number; bottom: number; kind: RegionKind };

type Layout = Exclude<TapZones, 'auto' | 'off'>;

const LAYOUTS: Record<Layout, Region[]> = {
  // +---+---+---+
  // | P | P | P |
  // | P | M | N |
  // | N | N | N |
  lShaped: [
    { left: 0, top: 0.33, right: 0.33, bottom: 0.66, kind: 'previous' },
    { left: 0, top: 0, right: 1, bottom: 0.33, kind: 'previous' },
    { left: 0.66, top: 0.33, right: 1, bottom: 0.66, kind: 'next' },
    { left: 0, top: 0.66, right: 1, bottom: 1, kind: 'next' },
  ],
  // | M | M | M |
  // | P | N | N |
  // | P | N | N |
  kindle: [
    { left: 0.33, top: 0.33, right: 1, bottom: 1, kind: 'next' },
    { left: 0, top: 0.33, right: 0.33, bottom: 1, kind: 'previous' },
  ],
  // | N | N | N |
  // | N | M | N |
  // | N | P | N |
  edge: [
    { left: 0, top: 0, right: 0.33, bottom: 1, kind: 'next' },
    { left: 0.33, top: 0.66, right: 0.66, bottom: 1, kind: 'previous' },
    { left: 0.66, top: 0, right: 1, bottom: 1, kind: 'next' },
  ],
  // | L | M | R |
  sides: [
    { left: 0, top: 0, right: 0.33, bottom: 1, kind: 'left' },
    { left: 0.66, top: 0, right: 1, bottom: 1, kind: 'right' },
  ],
};

export const TAP_ZONES: TapZones[] = ['auto', 'lShaped', 'kindle', 'edge', 'sides', 'off'];
export const TAP_INVERTS: TapInvert[] = ['none', 'horizontal', 'vertical', 'both'];

/** The layout a mode uses: Auto is the sides for pages and the L shape for the strip (Mihon's). */
export const resolveZones = (zones: TapZones, mode: ReaderMode): Layout | 'off' => {
  if (zones !== 'auto') return zones;
  return mode === 'webtoon' ? 'lShaped' : 'sides';
};

const mirrored = (region: Region, invert: TapInvert): Region => {
  const horizontal = invert === 'horizontal' || invert === 'both';
  const vertical = invert === 'vertical' || invert === 'both';
  return {
    kind: region.kind,
    left: horizontal ? 1 - region.right : region.left,
    right: horizontal ? 1 - region.left : region.right,
    top: vertical ? 1 - region.bottom : region.top,
    bottom: vertical ? 1 - region.top : region.bottom,
  };
};

/** The zones of a layout, mirrored as set, for drawing them. */
export const zoneRegions = (zones: TapZones, mode: ReaderMode, invert: TapInvert): Region[] => {
  const layout = resolveZones(zones, mode);
  return layout === 'off' ? [] : LAYOUTS[layout].map((region) => mirrored(region, invert));
};

/**
 * What a tap at (`x`, `y`) does, both fractions of the reading area. Right to left books swap
 * what the left and right sides do.
 */
export const tapAction = (
  zones: TapZones,
  invert: TapInvert,
  mode: ReaderMode,
  x: number,
  y: number,
): TapAction => {
  const region = zoneRegions(zones, mode, invert).find(
    (zone) => x >= zone.left && x < zone.right && y >= zone.top && y < zone.bottom,
  );
  if (!region) return 'menu';
  const rtl = mode === 'pagedRtl';
  switch (region.kind) {
    case 'previous':
      return 'back';
    case 'next':
      return 'forward';
    case 'left':
      return rtl ? 'forward' : 'back';
    case 'right':
      return rtl ? 'back' : 'forward';
  }
};
