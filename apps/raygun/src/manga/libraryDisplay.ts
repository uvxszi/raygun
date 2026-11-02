import type {
  LibraryDisplayMode,
  LibraryDisplayPrefs,
  LibraryList,
  LibraryTabs,
} from '@raygun/bindings';

/** Every field of the library display: Rust sends them all, the generated type has them optional. */
export type LibraryDisplay = Required<LibraryDisplayPrefs>;

/** Matches `LibraryDisplayPrefs::default()` in raygun-model. */
export const LIBRARY_DISPLAY_DEFAULTS: LibraryDisplay = {
  mode: 'compactGrid',
  columnsLandscape: 0,
  tabs: 'categories',
  showCounts: true,
  perCategory: false,
  continueButton: true,
  localBadge: true,
  pinsOnTop: true,
};

export const withLibraryDisplay = (prefs: LibraryDisplayPrefs | undefined): LibraryDisplay => ({
  ...LIBRARY_DISPLAY_DEFAULTS,
  ...prefs,
});

export const DISPLAY_MODES: LibraryDisplayMode[] = [
  'compactGrid',
  'comfortableGrid',
  'coverOnlyGrid',
  'list',
];
export const TAB_MODES: LibraryTabs[] = ['categories', 'lists', 'none'];
/** CloudStream's order: what you're on, then what's done or set aside, then what's next. */
export const LISTS: LibraryList[] = ['reading', 'completed', 'onHold', 'dropped', 'planned'];
/** Matches `GRID_COLUMNS` in raygun-model (0 fits covers to the screen). */
export const GRID_COLUMNS = [0, 2, 3, 4, 5, 6, 7, 8];

/** Words that read differently for videos ("Watching", "Plan to watch"). */
export const listKey = (list: LibraryList, video: boolean) =>
  video && (list === 'reading' || list === 'planned')
    ? `library.lists.video.${list}`
    : `library.lists.${list}`;
