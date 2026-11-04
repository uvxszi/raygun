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
/** Matches `COVER_SIZES` in raygun-model: percent more or less than the screen's own (M46.48). */
export const COVER_SIZE = { min: -50, max: 100, step: 5 } as const;

/** Words that read differently for videos ("Watching", "Plan to watch"). */
export const listKey = (list: LibraryList, video: boolean) =>
  video && (list === 'reading' || list === 'planned')
    ? `library.lists.video.${list}`
    : `library.lists.${list}`;
