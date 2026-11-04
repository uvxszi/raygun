import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';

/** What an extension is for, as the Browse filters group it. */
export type ExtensionKind =
  'music' | 'manga' | 'novel' | 'video' | 'podcast' | 'audiobook' | 'lyrics' | 'other';

export const EXTENSION_KINDS: ExtensionKind[] = [
  'music',
  'manga',
  'novel',
  'video',
  'podcast',
  'audiobook',
  'lyrics',
  'other',
];

const kindOf = (capability: string): ExtensionKind => {
  if (capability.startsWith('music.')) return 'music';
  if (capability === 'manga.source') return 'manga';
  if (capability === 'novel.source') return 'novel';
  if (capability.startsWith('video.') || capability.startsWith('stremio.')) return 'video';
  if (capability.startsWith('podcast.')) return 'podcast';
  if (capability.startsWith('audiobook.')) return 'audiobook';
  if (capability === 'lyrics') return 'lyrics';
  return 'other';
};

/** The kinds an extension (installed, or listed by a repository) provides. */
export const kindsOf = (entry: { provides: string[] }): ExtensionKind[] => {
  const kinds = new Set(entry.provides.map(kindOf));
  return kinds.size ? EXTENSION_KINDS.filter((kind) => kinds.has(kind)) : ['other'];
};

/** Which of a repository's extensions Browse → Extensions lists. */
export type CatalogShow = 'all' | 'notInstalled' | 'updates';

export type CatalogFilter = {
  query: string;
  /** Only extensions of this kind; every kind while null. */
  kind: ExtensionKind | null;
  lang: string | null;
  repoId: string | null;
};

export const NO_FILTER: CatalogFilter = {
  query: '',
  kind: null,
  lang: null,
  repoId: null,
};

const shows = (entry: CatalogEntry, show: CatalogShow) =>
  show === 'all' ||
  (show === 'notInstalled' && entry.state === 'available') ||
  (show === 'updates' && entry.state === 'updateAvailable');

const matches = (entry: CatalogEntry, filter: CatalogFilter, show: CatalogShow) => {
  const query = filter.query.trim().toLowerCase();
  if (
    query &&
    !entry.name.toLowerCase().includes(query) &&
    !(entry.description ?? '').toLowerCase().includes(query)
  ) {
    return false;
  }
  if (filter.kind && !kindsOf(entry).includes(filter.kind)) return false;
  if (!shows(entry, show)) return false;
  // Entries without a language (or with many) serve every language.
  if (filter.lang && entry.lang && entry.lang !== 'multi' && entry.lang !== filter.lang) {
    return false;
  }
  return true;
};

/** The catalog with only matching entries; repositories left empty by the filter are dropped. */
export const filterCatalog = (
  catalog: RepoCatalog[],
  filter: CatalogFilter,
  show: CatalogShow = 'all',
): RepoCatalog[] => {
  const filtering =
    show !== 'all' || (filter !== NO_FILTER && (filter.query.trim() || filter.kind || filter.lang));
  return catalog
    .filter((listing) => !filter.repoId || listing.repo.id === filter.repoId)
    .map((listing) => ({
      ...listing,
      entries: listing.entries.filter((entry) => matches(entry, filter, show)),
    }))
    .filter((listing) => !filtering || listing.entries.length > 0 || listing.error);
};

/** How many of the catalog's entries can be updated. */
export const updateCount = (catalog: RepoCatalog[] | undefined) =>
  (catalog ?? []).reduce(
    (count, listing) =>
      count + listing.entries.filter((entry) => entry.state === 'updateAvailable').length,
    0,
  );

/** The kinds and languages present, for the filter controls. */
export const catalogFacets = (catalog: RepoCatalog[]) => {
  const kinds = new Set<ExtensionKind>();
  const langs = new Set<string>();
  for (const listing of catalog) {
    for (const entry of listing.entries) {
      for (const kind of kindsOf(entry)) kinds.add(kind);
      if (entry.lang) langs.add(entry.lang);
    }
  }
  return {
    kinds: EXTENSION_KINDS.filter((kind) => kinds.has(kind)),
    langs: [...langs].sort((a, b) => a.localeCompare(b)),
  };
};

/** Extensions that came from another app's format, which a repository may hold. */
export type RepoFormat = 'nuclear' | 'shosetsu' | 'miru';

const formatOf = (entry: CatalogEntry): RepoFormat | null =>
  entry.nuclear
    ? 'nuclear'
    : entry.id.startsWith('shosetsu.')
      ? 'shosetsu'
      : entry.id.startsWith('miru.')
        ? 'miru'
        : null;

/**
 * What a repository holds: how many extensions of each kind (an extension with several kinds
 * counts in each), and how many are another app's format (Nuclear plugins, Shosetsu and Miru
 * scripts).
 */
export const repoContents = (entries: CatalogEntry[]) => {
  const kinds = new Map<ExtensionKind, number>();
  const formats = new Map<RepoFormat, number>();
  for (const entry of entries) {
    for (const kind of kindsOf(entry)) kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
    const format = formatOf(entry);
    if (format) formats.set(format, (formats.get(format) ?? 0) + 1);
  }
  return {
    kinds: EXTENSION_KINDS.filter((kind) => kinds.has(kind)).map((kind) => ({
      kind,
      count: kinds.get(kind) ?? 0,
    })),
    formats: (['nuclear', 'shosetsu', 'miru'] as const)
      .filter((format) => formats.has(format))
      .map((format) => ({ format, count: formats.get(format) ?? 0 })),
  };
};
