import type { LayoutItem, LayoutPrefs } from '@raygun/bindings';

/**
 * How the app's screens are arranged (M46.5): each list in the user's order, each entry shown or
 * hidden. Rust stores and checks it (`LayoutPrefs` in raygun-model); this side fills in what an
 * older stored row lacks, the same way, so the UI never sees a partial list.
 */
export type Layout = Required<LayoutPrefs>;
export type LayoutSurface = keyof Layout;

/** Matches `LAYOUT_*` in raygun-model: every entry in its default order, and whether it shows. */
export const LAYOUT_ENTRIES: Record<LayoutSurface, readonly (readonly [string, boolean])[]> = {
  tabs: [
    ['home', true],
    ['library', true],
    ['browse', true],
    ['more', true],
    ['watch', false],
    ['music', false],
    ['comics', false],
    ['books', false],
    ['podcasts', false],
    ['search', false],
    ['updates', false],
    ['downloads', false],
    ['settings', false],
  ],
  libraryMedia: [
    ['music', true],
    ['manga', true],
    ['novel', true],
    ['video', true],
  ],
  audioViews: [
    ['all', true],
    ['podcasts', true],
    ['playlists', true],
    ['favorites', true],
    ['albums', true],
    ['artists', true],
    ['tracks', true],
  ],
  homeSections: [
    ['continueWatching', true],
    ['continueReading', true],
    ['yourMusic', true],
    ['recentlyPlayed', true],
  ],
  browseTabs: [
    ['search', true],
    ['sources', true],
    ['extensions', true],
  ],
  browseSections: [
    ['video', true],
    ['manga', true],
    ['novel', true],
    ['websites', true],
  ],
  moreEntries: [
    ['updates', true],
    ['downloads', true],
    ['categories', true],
    ['addons', true],
    ['repos', true],
    ['ai', true],
    ['devices', true],
    ['settings', true],
    ['about', true],
  ],
};

/** Entries that can't be hidden: More and Settings lead everywhere else; Browse keeps Search. */
export const LOCKED: Partial<Record<LayoutSurface, string[]>> = {
  tabs: ['more'],
  moreEntries: ['settings'],
  browseTabs: ['search'],
};

export const LAYOUT_SURFACES = Object.keys(LAYOUT_ENTRIES) as LayoutSurface[];

export const isLayoutSurface = (value: unknown): value is LayoutSurface =>
  typeof value === 'string' && value in LAYOUT_ENTRIES;

/** `saved` in its order with only known ids, then the missing defaults after their neighbour. */
const arrange = (
  saved: LayoutItem[] | undefined,
  defaults: readonly (readonly [string, boolean])[],
): LayoutItem[] => {
  const items: LayoutItem[] = [];
  for (const item of saved ?? []) {
    const known = defaults.some(([id]) => id === item.id);
    if (known && !items.some((other) => other.id === item.id)) items.push({ ...item });
  }
  defaults.forEach(([id, visible], index) => {
    if (items.some((item) => item.id === id)) return;
    let at = 0;
    for (let before = index - 1; before >= 0; before -= 1) {
      const position = items.findIndex((item) => item.id === defaults[before]![0]);
      if (position >= 0) {
        at = position + 1;
        break;
      }
    }
    items.splice(at, 0, { id, visible });
  });
  return items;
};

/** Every list whole, in the user's order where they set one. */
export const withLayout = (layout: LayoutPrefs | undefined): Layout =>
  Object.fromEntries(
    LAYOUT_SURFACES.map((surface) => [
      surface,
      arrange(layout?.[surface], LAYOUT_ENTRIES[surface]),
    ]),
  ) as Layout;

export const LAYOUT_DEFAULTS: Layout = withLayout(undefined);
