import type { MangaReaderPrefs, PagedScale, ReaderMode } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

export type ReaderDisplay = Required<MangaReaderPrefs>;

/** Matches `MangaReaderPrefs::default()` in raygun-model. */
export const READER_DISPLAY_DEFAULTS: ReaderDisplay = {
  pagedScale: 'fitScreen',
  webtoonWidth: 0,
  barWidth: 0,
  barChapters: true,
  barZoom: true,
  barMode: true,
  barFullscreen: true,
  preloadPages: 3,
  defaultMode: 'webtoon',
  tapZones: 'auto',
  invertTaps: 'none',
  doubleTapZoom: true,
  pageNumber: true,
  keepScreenOn: true,
  skipDuplicates: true,
};

export const PAGED_SCALES: PagedScale[] = ['fitScreen', 'fitWidth', 'fitHeight', 'original'];
/** The reading modes, in the order the reader offers them. */
export const READER_MODES: ReaderMode[] = ['webtoon', 'pagedRtl', 'pagedLtr'];
/** Webtoon column widths offered (0: by screen). */
export const WEBTOON_WIDTHS = [0, 30, 40, 50, 60, 70, 80, 90, 100];
/** Page bar widths offered (0: by screen). */
export const BAR_WIDTHS = [0, 40, 50, 60, 70, 80, 90, 100];

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
