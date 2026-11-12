import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';

import {
  EXTENSION_KINDS,
  NO_FILTER,
  filterCatalog,
  isNotForRaygun,
  kindsOf,
  repoContents,
  repoFormat,
  updateCount,
} from './catalogFilters';
import type { CatalogShow } from './catalogFilters';

describe('kindsOf', () => {
  it('puts data sources under Data as well as what they are for', () => {
    expect(kindsOf({ provides: ['music.metadata', 'music.tags'] })).toEqual(['music', 'data']);
    expect(kindsOf({ provides: ['lyrics'] })).toEqual(['lyrics', 'data']);
  });

  it('leaves sources of content out of Data', () => {
    expect(kindsOf({ provides: ['manga.source'] })).toEqual(['manga']);
    expect(kindsOf({ provides: ['music.stream'] })).toEqual(['music']);
    expect(kindsOf({ provides: ['music.metadata', 'music.stream', 'video.source'] })).toEqual([
      'music',
      'video',
    ]);
  });

  it('calls an extension that provides nothing known Other', () => {
    expect(kindsOf({ provides: [] })).toEqual(['other']);
  });

  it('lists Data before Other', () => {
    expect(EXTENSION_KINDS.indexOf('data')).toBeLessThan(EXTENSION_KINDS.indexOf('other'));
  });
});

const entry = (overrides: Partial<CatalogEntry> = {}): CatalogEntry => ({
  repoId: 'repo-1',
  id: 'dev.example.radio',
  name: 'Radio Stations',
  version: '1.0.0',
  description: null,
  author: null,
  provides: ['music.metadata'],
  net: [],
  contentRating: 'safe',
  adult: false,
  lang: null,
  size: 1,
  state: 'available',
  installedVersion: null,
  nuclear: false,
  icon: null,
  ...overrides,
});

const notForRaygun = (overrides: Partial<CatalogEntry> = {}) =>
  entry({
    id: 'kodi.skin.example',
    name: 'Example skin',
    provides: [],
    unsupported: "A skin for Kodi's own screens; Raygun has its own look",
    unsupportedKind: 'notForRaygun',
    ...overrides,
  });

const listing = (entries: CatalogEntry[]): RepoCatalog[] => [
  {
    repo: { id: 'repo-1', url: 'https://example.org/index.json', name: 'Repo' },
    entries,
    error: null,
  },
];

describe('entries not for Raygun', () => {
  it('are only listed under All', () => {
    const catalog = listing([entry(), notForRaygun()]);
    const names = (show: CatalogShow) =>
      filterCatalog(catalog, NO_FILTER, show).flatMap((shown) =>
        shown.entries.map((item) => item.name),
      );
    expect(names('all')).toEqual(['Radio Stations', 'Example skin']);
    expect(names('notInstalled')).toEqual(['Radio Stations']);
    expect(isNotForRaygun(notForRaygun())).toBe(true);
    expect(isNotForRaygun(entry({ unsupported: 'Kodi', unsupportedKind: 'cantRun' }))).toBe(false);
  });

  it('are never counted as updates, nor is any update that can’t be installed', () => {
    const catalog = listing([
      entry({ state: 'updateAvailable' }),
      notForRaygun({ id: 'mihon.en.empty', state: 'updateAvailable' }),
      entry({ id: 'mihon.en.x', state: 'updateAvailable', unsupported: 'Raygun can’t convert it' }),
    ]);
    expect(updateCount(catalog)).toBe(1);
    expect(filterCatalog(catalog, NO_FILTER, 'updates')[0]?.entries.map((item) => item.id)).toEqual(
      ['dev.example.radio'],
    );
  });

  it('hold nothing a repository is counted for', () => {
    const contents = repoContents([
      entry({ id: 'kodi.plugin.video.x', provides: ['video.source'] }),
      notForRaygun(),
    ]);
    expect(contents.kinds).toEqual([{ kind: 'video', count: 1 }]);
    expect(contents.formats).toEqual([{ format: 'kodi', count: 1 }]);
  });
});

describe('Nuvio repositories', () => {
  it('are read as Nuvio’s, their providers as video', () => {
    const providers = [
      entry({ id: 'nuvio.archive', provides: ['video.streams'] }),
      entry({ id: 'nuvio.old-tv', provides: ['video.streams'] }),
    ];
    expect(repoFormat(providers)).toBe('nuvio');
    expect(repoContents(providers)).toEqual({
      kinds: [{ kind: 'video', count: 2 }],
      formats: [{ format: 'nuvio', count: 2 }],
    });
  });
});
