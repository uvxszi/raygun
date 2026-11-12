import type { MangaReaderPrefs, PagedScale, ReaderMode } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { DEFAULT_GRIDS, toGrid } from './reader/tapZones';

export type ReaderDisplay = Required<MangaReaderPrefs>;

/** Matches `MangaReaderPrefs::default()` in raygun-model. */
export const READER_DISPLAY_DEFAULTS: ReaderDisplay = {
  pagedScale: 'fitScreen',
  webtoonSize: 0,
  barSize: 0,
  barProgress: true,
  barChapters: true,
  barZoom: true,
  barMode: true,
  barFullscreen: true,
  preloadPages: 3,
  defaultMode: 'webtoon',
  tapZones: 'auto',
  tapGridPaged: toGrid(DEFAULT_GRIDS.paged),
  tapGridWebtoon: toGrid(DEFAULT_GRIDS.webtoon),
  invertTaps: 'none',
  doubleTapZoom: true,
  pageNumber: true,
  keepScreenOn: true,
  fullScreen: true,
  skipDuplicates: true,
};

export const PAGED_SCALES: PagedScale[] = ['fitScreen', 'fitWidth', 'fitHeight', 'original'];
/**
 * The webtoon column's and the page bar's sizes (M46.48): percent more or less than the screen's
 * own (`READER_SIZES` in raygun-model).
 */
export const READER_SIZE = { min: -70, max: 100, step: 5 } as const;

/** `percent` more or less than `base` (a CSS length), never wider than what holds it. */
export const sizedFrom = (base: string, percent: number) =>
  percent === 0 ? `min(100%, ${base})` : `min(100%, calc(${base} * ${1 + percent / 100}))`;
/** `percent` more or less than `base`, never wider than the reader's screen (a size container). */
const screenSized = (base: string, percent: number) =>
  percent === 0 ? `min(100cqw, ${base})` : `min(100cqw, calc(${base} * ${1 + percent / 100}))`;

/**
 * The strip's width: exactly the reader's screen's (the screen's, on phones) up to 768 px at the
 * fit, then zoomed. In the screen's own units (`cqw`, its scroller is the size container) rather
 * than the viewport's, so a scrollbar or safe area beside it never pushes the right edge off
 * screen, and a zoomed strip wider than the screen keeps its width.
 */
export const stripWidth = (size: number, zoom: number) =>
  zoom === 1 ? screenSized('48rem', size) : `calc(${screenSized('48rem', size)} * ${zoom})`;

/**
 * Room above the strip's first page (`ratio` its width over its height), so a page shorter than
 * the reader's screen starts in its middle, as the paged modes show it; none for a taller page.
 * Outside the zoomed strip, but following its zoom.
 */
export const stripLead = (size: number, zoom: number, ratio: number) =>
  `max(0px, calc((100cqh - ${stripWidth(size, zoom)} / ${ratio}) / 2))`;

/** The reading modes, in the order the reader offers them. */
export const READER_MODES: ReaderMode[] = ['webtoon', 'pagedLtr', 'pagedRtl'];

/** Zoom steps in the reader: Ctrl+scroll, Ctrl+plus/minus and the bar's buttons. */
export const ZOOM_STEP = 0.1;
export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 4;

export const clampZoom = (zoom: number) =>
  Math.round(Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom)) * 100) / 100;

/** The manga reader's display settings (M46.7), whole, and a way to change some of them. */
export const useReaderDisplay = () => {
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const display: ReaderDisplay = { ...READER_DISPLAY_DEFAULTS, ...settings?.mangaReader };
  const set = (patch: Partial<ReaderDisplay>) =>
    update.mutate({ key: 'mangaReader', value: { ...display, ...patch } });
  return { display, set, busy: update.isPending, ready: settings !== undefined };
};
