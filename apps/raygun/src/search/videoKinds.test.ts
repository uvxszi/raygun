import type { StremioCatalogRow, StremioItem } from '@raygun/bindings';

import { mergeKinds } from './videoKinds';

const item = (
  kind: string,
  id: string,
  name: string,
  releaseInfo: string | null = null,
  extra: Partial<StremioItem> = {},
): StremioItem => ({
  kind,
  id,
  name,
  poster: null,
  posterShape: 'poster',
  releaseInfo,
  rating: null,
  ...extra,
});

const row = (addonId: string, kind: string, items: StremioItem[]): StremioCatalogRow => ({
  addonId,
  addonName: addonId.charAt(0).toUpperCase() + addonId.slice(1),
  kind,
  catalogId: 'search',
  name: 'Search',
  items,
  genres: [],
  hasMore: false,
  error: null,
});

/** Each section as its kind and titles, each title as its copies' `addon id`. */
const shape = (rows: StremioCatalogRow[], query: string) =>
  mergeKinds(rows, query).map((section) => ({
    kind: section.key,
    titles: section.titles.map((title) =>
      title.copies.map((copy) => `${copy.addonId} ${copy.item.id}`),
    ),
  }));

describe('mergeKinds', () => {
  it('makes one section per kind, in the order the kinds first come, whatever their case', () => {
    const sections = mergeKinds(
      [
        row('cinemeta', 'series', [item('series', 'tt1', 'Dark')]),
        row('cinemeta', 'movie', [item('movie', 'tt2', 'Dark Waters')]),
        row('kitsu', 'Anime', [item('Anime', 'kitsu:1', 'Darker than Black')]),
        row('anilist', 'anime', [item('anime', 'anilist:2', 'Dark Gathering')]),
        row('torrentio', 'series', [item('series', 'tt3', 'Dark Matter')]),
      ],
      'dark',
    );
    expect(sections.map((section) => [section.key, section.kind])).toEqual([
      ['series', 'series'],
      ['movie', 'movie'],
      ['anime', 'Anime'],
    ]);
    expect(sections[0]!.titles.map((title) => title.item.name)).toEqual(['Dark', 'Dark Matter']);
    // Each copy keeps its kind as its addon wrote it: its page is asked with that.
    expect(sections[2]!.titles.map((title) => title.item.kind)).toEqual(['Anime', 'anime']);
  });

  it("ranks every addon's titles together: best answers first, then places, then addons", () => {
    expect(
      shape(
        [
          row('first', 'series', [
            item('series', 'a1', 'Office Space'),
            item('series', 'a2', 'My Office Life'),
          ]),
          row('second', 'series', [
            item('series', 'b1', 'Home Office Stories'),
            item('series', 'b2', 'The Office'),
          ]),
        ],
        'the office',
      ),
    ).toEqual([
      {
        kind: 'series',
        titles: [['second b2'], ['first a1'], ['second b1'], ['first a2']],
      },
    ]);
  });

  it('shows a title several addons have once, the best copy first', () => {
    const sections = mergeKinds(
      [
        row('cinemeta', 'series', [
          item('series', 'tt0903747', 'Breaking Bad', '2008–2013', { poster: null }),
        ]),
        row('torrentio', 'series', [
          item('series', 'tt0903747', 'Breaking Bad', '2008–2013', {
            poster: 'https://img.example/bb.jpg',
            rating: '9.5',
          }),
        ]),
        row('kitsu', 'series', [item('series', 'tt1', 'Breaking Point', '2020')]),
      ],
      'breaking bad',
    );
    const [breaking, other] = sections[0]!.titles;
    expect(breaking!.copies.map((copy) => copy.addonName)).toEqual(['Cinemeta', 'Torrentio']);
    // The card opens the best copy; a poster and rating another addon has fill its gaps.
    expect(breaking!.item).toMatchObject({
      id: 'tt0903747',
      poster: 'https://img.example/bb.jpg',
      rating: '9.5',
    });
    expect(other!.copies).toHaveLength(1);
  });

  it('joins copies with ids of their own by name, season and year', () => {
    expect(
      shape(
        [
          row('cinemeta', 'series', [
            item('series', 'tt22248376', 'Frieren', '2023'),
            item('series', 'tt0001', 'Frieren', '1999'),
            item('series', 'tt0002', 'Frieren Season 2', '2026'),
          ]),
          row('kitsu', 'series', [
            // A year apart is the same release.
            item('series', 'kitsu:46474', 'Frieren', '2024'),
            item('series', 'kitsu:2', 'Frieren 2nd Season', '2026'),
            // Without a year it can't be told apart.
            item('series', 'kitsu:3', 'Frieren'),
          ]),
        ],
        'frieren',
      ),
    ).toEqual([
      {
        kind: 'series',
        titles: [
          ['cinemeta tt22248376', 'kitsu kitsu:46474'],
          ['cinemeta tt0001'],
          ['kitsu kitsu:3'],
          // The seasons after the first rank after it, Kitsu's "2nd Season" leading its own.
          ['kitsu kitsu:2', 'cinemeta tt0002'],
        ],
      },
    ]);
  });

  it("never joins an addon's titles with each other, and counts its own copies once", () => {
    expect(
      shape(
        [
          row('cinemeta', 'movie', [
            item('movie', 'tt1', 'Dune', '2021'),
            item('movie', 'tt2', 'Dune', '2021'),
          ]),
          // The same addon's other catalog lists one again.
          row('cinemeta', 'movie', [item('movie', 'tt1', 'Dune', '2021')]),
          row('tmdb', 'movie', [item('movie', 'tmdb:438631', 'Dune', '2021')]),
        ],
        'dune',
      ),
    ).toEqual([
      {
        kind: 'movie',
        titles: [['cinemeta tt1', 'tmdb tmdb:438631'], ['cinemeta tt2']],
      },
    ]);
  });
});
