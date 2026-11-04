// The tap zone layouts are adapted from Mihon (Apache-2.0),
// app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/*.kt and ViewerNavigation.kt.

import type { MangaReaderPrefs, ReaderMode, TapCell, TapInvert, TapZones } from '@raygun/bindings';

/** What a tap does: show or hide the controls, turn or scroll, jump, or nothing. */
export type TapAction =
  | 'menu'
  | 'forward'
  | 'back'
  | 'firstPage'
  | 'lastPage'
  | 'previousChapter'
  | 'nextChapter'
  | 'nothing';

/** Nine cells, row by row from the top left (`TapGrid` in raygun-model). */
export type TapGridCells = readonly TapCell[];
/** The stored form: exactly nine cells. */
export type TapGrid = NonNullable<MangaReaderPrefs['tapGridPaged']>;
export const toGrid = (cells: TapGridCells) => [...cells] as unknown as TapGrid;

type Preset = Exclude<TapZones, 'auto' | 'custom'>;

const [M, P, N, L, R] = ['menu', 'previous', 'next', 'left', 'right'] as const;

export const TAP_PRESETS: Record<Preset, TapGridCells> = {
  // Top row and the middle's left go back; the middle's right and the bottom row go on.
  lShaped: [P, P, P, P, M, N, N, N, N],
  // The top shows the controls; the left third goes back, the rest on.
  kindle: [M, M, M, P, N, N, P, N, N],
  // Both sides go on; the bottom of the middle goes back.
  edge: [N, M, N, N, M, N, N, P, N],
  // The left and right thirds, as the reading direction has them.
  sides: [L, M, R, L, M, R, L, M, R],
  off: [M, M, M, M, M, M, M, M, M],
};

/** The presets in the order the grid editor offers them. */
export const TAP_PRESET_NAMES: Preset[] = ['lShaped', 'kindle', 'edge', 'sides', 'off'];

export const TAP_ZONES: TapZones[] = [
  'auto',
  'lShaped',
  'kindle',
  'edge',
  'sides',
  'off',
  'custom',
];
export const TAP_INVERTS: TapInvert[] = ['none', 'horizontal', 'vertical', 'both'];
export const TAP_CELLS: TapCell[] = [
  'menu',
  'previous',
  'next',
  'left',
  'right',
  'firstPage',
  'lastPage',
  'previousChapter',
  'nextChapter',
  'nothing',
];

/** Which of the two custom grids a mode uses. */
export type GridKind = 'paged' | 'webtoon';
export const gridKind = (mode: ReaderMode): GridKind => (mode === 'webtoon' ? 'webtoon' : 'paged');

/** Each kind's own default: the sides for pages and the L shape for the strip (Mihon's). */
export const DEFAULT_GRIDS: Record<GridKind, TapGridCells> = {
  paged: TAP_PRESETS.sides,
  webtoon: TAP_PRESETS.lShaped,
};

type GridPrefs = Pick<MangaReaderPrefs, 'tapZones' | 'tapGridPaged' | 'tapGridWebtoon'>;

/** The grid a kind of reading uses with these settings. */
export const gridOf = (prefs: GridPrefs, kind: GridKind): TapGridCells => {
  const zones = prefs.tapZones ?? 'auto';
  if (zones === 'auto') return DEFAULT_GRIDS[kind];
  if (zones !== 'custom') return TAP_PRESETS[zones];
  const own = kind === 'webtoon' ? prefs.tapGridWebtoon : prefs.tapGridPaged;
  return own && own.length === 9 ? own : DEFAULT_GRIDS[kind];
};

const third = (fraction: number) => Math.min(2, Math.max(0, Math.floor(fraction * 3)));

/** The cell (0–8) a tap at (`x`, `y`) falls in, both fractions of the area, mirrored as set. */
export const cellAt = (invert: TapInvert, x: number, y: number) => {
  const column = third(x);
  const row = third(y);
  const horizontal = invert === 'horizontal' || invert === 'both';
  const vertical = invert === 'vertical' || invert === 'both';
  return (vertical ? 2 - row : row) * 3 + (horizontal ? 2 - column : column);
};

/**
 * What a tap at (`x`, `y`) does, both fractions of the reading area. Right to left books swap what
 * Left and Right do.
 */
export const tapAction = (
  grid: TapGridCells,
  invert: TapInvert,
  mode: ReaderMode,
  x: number,
  y: number,
): TapAction => {
  const cell = grid[cellAt(invert, x, y)] ?? 'menu';
  const rtl = mode === 'pagedRtl';
  switch (cell) {
    case 'previous':
      return 'back';
    case 'next':
      return 'forward';
    case 'left':
      return rtl ? 'forward' : 'back';
    case 'right':
      return rtl ? 'back' : 'forward';
    default:
      return cell;
  }
};
