import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';

/** What an extension is for, as the Browse filters group it. */
export type ExtensionKind = 'music' | 'manga' | 'novel' | 'video' | 'podcast' | 'lyrics' | 'other';

export const EXTENSION_KINDS: ExtensionKind[] = [
  'music',
  'manga',
  'novel',
  'video',
  'podcast',
  'lyrics',
  'other',
];

const kindOf = (capability: string): ExtensionKind => {
  if (capability.startsWith('music.')) return 'music';
  if (capability === 'manga.source') return 'manga';
  if (capability === 'novel.source') return 'novel';
  if (capability.startsWith('video.') || capability.startsWith('stremio.')) return 'video';
  if (capability.startsWith('podcast.')) return 'podcast';
  if (capability === 'lyrics') return 'lyrics';
  return 'other';
};

export const kindsOf = (entry: CatalogEntry): ExtensionKind[] => {
  const kinds = new Set(entry.provides.map(kindOf));
  return kinds.size ? [...kinds] : ['other'];
};

export type CatalogFilter = {
  query: string;
  kind: ExtensionKind | null;
  lang: string | null;
  repoId: string | null;
};

export const NO_FILTER: CatalogFilter = { query: '', kind: null, lang: null, repoId: null };

const matches = (entry: CatalogEntry, filter: CatalogFilter) => {
  const query = filter.query.trim().toLowerCase();
  if (
    query &&
    !entry.name.toLowerCase().includes(query) &&
    !(entry.description ?? '').toLowerCase().includes(query)
  ) {
    return false;
  }
  if (filter.kind && !kindsOf(entry).includes(filter.kind)) return false;
  if (filter.lang && entry.lang !== filter.lang) return false;
  return true;
};

/** The catalog with only matching entries; repositories left empty by the filter are dropped. */
export const filterCatalog = (catalog: RepoCatalog[], filter: CatalogFilter): RepoCatalog[] => {
  const filtering = filter !== NO_FILTER && (filter.query.trim() || filter.kind || filter.lang);
  return catalog
    .filter((listing) => !filter.repoId || listing.repo.id === filter.repoId)
    .map((listing) => ({
      ...listing,
      entries: listing.entries.filter((entry) => matches(entry, filter)),
    }))
    .filter((listing) => !filtering || listing.entries.length > 0 || listing.error);
};

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
