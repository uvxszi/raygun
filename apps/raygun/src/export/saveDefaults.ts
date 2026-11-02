import type { SavePrefs } from '@raygun/bindings';

export type Save = Required<SavePrefs>;

export const SAVE_DEFAULTS: Save = {
  location: 'app',
  folderUri: null,
  folderName: null,
  chapterFormat: 'cbz',
  pagesPerTitle: true,
};

/** Settings → Downloads with every field filled (the stored row may predate some). */
export const withSave = (prefs: SavePrefs | undefined): Save => ({ ...SAVE_DEFAULTS, ...prefs });
