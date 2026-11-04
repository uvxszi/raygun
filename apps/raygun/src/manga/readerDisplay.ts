import type { MangaReaderPrefs, PagedScale } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

export type ReaderDisplay = Required<MangaReaderPrefs>;

/** Matches `MangaReaderPrefs::default()` in raygun-model. */
export const READER_DISPLAY_DEFAULTS: ReaderDisplay = {
  pagedScale: 'fitScreen',
  webtoonSize: 0,
  barSize: 0,
  barChapters: true,
  barZoom: true,
  barMode: true,
  barFullscreen: true,
  preloadPages: 3,
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
