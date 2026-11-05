import type { RailKinds, RailOrder, RailPin, RailPrefs } from '@raygun/bindings';

/** Quick access's pages and settings as plain data (no bindings calls: tests' mocks use it). */

export type PinTarget = Pick<RailPin, 'kind' | 'to' | 'search' | 'name' | 'artworkUrl'>;

export type Page = Pick<RailPin, 'to' | 'search'>;

/** Two pins are the same page: same route and same search parameters. */
export const samePin = (a: Page, b: Page) =>
  a.to === b.to &&
  Object.keys({ ...a.search, ...b.search }).every((key) => a.search[key] === b.search[key]);

/** A page as one string: its route and its search parameters in order. */
export const pageKey = (page: Page) =>
  `${page.to}?${Object.entries(page.search)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => `${key}=${value}`)
    .join('&')}`;

/** Favorites, Downloaded and Local files: their tiles' Pin to Quick access. */
export const FIXED_PINS: PinTarget[] = [
  { kind: 'favorites', to: '/library', search: { view: 'favorites' }, name: '', artworkUrl: null },
  {
    kind: 'downloaded',
    to: '/library',
    search: { view: 'downloaded' },
    name: '',
    artworkUrl: null,
  },
  { kind: 'localFiles', to: '/local-files', search: {}, name: '', artworkUrl: null },
];

export const playlistPin = (playlist: { id: string; name: string; artworkUrls: string[] }) => ({
  kind: 'playlist',
  to: '/library/playlist',
  search: { id: playlist.id },
  name: playlist.name,
  artworkUrl: playlist.artworkUrls[0] ?? null,
});

/** Quick access's settings with every field there. */
export type QuickAccessPrefs = Omit<Required<RailPrefs>, 'kinds'> & { kinds: Required<RailKinds> };

/** Quick access before the settings are read (Rust's defaults). */
export const RAIL_DEFAULTS: QuickAccessPrefs = {
  pins: [],
  order: 'played',
  seeded: false,
  autoAdd: true,
  limit: 20,
  drawerExpanded: true,
  shown: 100,
  kinds: {
    favorites: true,
    downloaded: true,
    localFiles: true,
    playlists: true,
    albums: true,
    artists: true,
    podcasts: true,
    audiobooks: true,
  },
  pinnedFirst: true,
  played: [],
  version: 2,
};

/** The orders quick access offers. */
export const RAIL_ORDERS: RailOrder[] = ['played', 'added', 'name'];

/** Listed at most: `RailPrefs::shown`'s limits in Rust. */
export const RAIL_SHOWN = { min: 10, max: 1000 } as const;

/** Every quick access list lives under this key; Rust's `QuickAccessChangedEvent` refreshes it. */
export const QUICK_ACCESS_KEY = ['quickAccess'] as const;

/** Quick access's settings with Rust's defaults filled in. */
export const withRailDefaults = (prefs: RailPrefs | undefined): QuickAccessPrefs => ({
  ...RAIL_DEFAULTS,
  ...prefs,
  kinds: { ...RAIL_DEFAULTS.kinds, ...prefs?.kinds },
});
