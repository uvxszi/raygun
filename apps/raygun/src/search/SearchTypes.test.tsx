import type { MangaCard, MangaLibraryEntry, SourceFilter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const COMICS = {
  id: 'dev.example.comics/comics',
  name: 'Comic Shelf',
  extensionId: 'dev.example.comics',
  enabled: true,
};
const OTHER = {
  id: 'dev.example.other/other',
  name: 'Other Source',
  extensionId: 'dev.example.other',
  enabled: true,
};
const card = (id: string, title: string, sourceId = COMICS.id): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
});

const FILTERS: SourceFilter[] = [
  {
    kind: 'select',
    key: 'status',
    label: 'Status',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      { value: 'completed', label: 'Completed' },
    ],
  },
  {
    kind: 'tags',
    key: 'tags',
    label: 'Tags',
    options: [
      { value: 't-action', label: 'Action' },
      { value: 't-horror', label: 'Horror' },
    ],
  },
  { kind: 'toggle', key: 'hasChapters', label: 'Has chapters', default: false },
];

describe('Search by type', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok([COMICS, OTHER]));
    // These tests look at a row per source; best matches have their own tests.
    localStorage.setItem('raygun.search.grouped.manga', '1');
    localStorage.setItem('raygun.search.grouped.novel', '1');
  });

  it('searches every kind and leads to each type', async () => {
    const user = userEvent.setup();
    commands.novelSources.mockResolvedValue(ok([OTHER]));
    commands.catalogGlobalSearch.mockImplementation(async (medium: string, _query, sources) =>
      ok(
        medium === 'manga' && sources[0] === COMICS.id
          ? [{ source: COMICS, items: [card('m1', 'Frieren')], hasNextPage: true, error: null }]
          : [],
      ),
    );
    commands.podcastSearch.mockResolvedValue(
      ok([
        {
          title: 'Frieren Talk',
          author: 'Fans',
          feedUrl: 'https://f.example/x.xml',
          artworkUrl: null,
          description: null,
          subscribed: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/search?q=frieren');
    const all = await screen.findByTestId('all-results');

    const manga = within(all).getByRole('region', { name: 'Comics' });
    expect(await within(manga).findByText('Frieren')).toBeInTheDocument();
    const podcasts = within(all).getByRole('region', { name: 'Podcasts' });
    expect(await within(podcasts).findByText('Frieren Talk')).toBeInTheDocument();
    // One source at a time, and only until two have something to show.
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', 'frieren', [COMICS.id]);
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('novel', 'frieren', [OTHER.id]);
    expect(
      commands.catalogGlobalSearch.mock.calls.every(([, , sources]) => sources.length === 1),
    ).toBe(true);

    await user.click(within(manga).getByRole('button', { name: 'See all' }));
    expect(screen.getByRole('tab', { name: 'Comics' })).toHaveAttribute('aria-selected', 'true');
    expect(await screen.findByTestId('manga-search')).toBeInTheDocument();
  });

  it('searches every manga source, then one source with filters and tags', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockResolvedValue(
      ok([
        { source: COMICS, items: [card('m1', 'Berserk')], hasNextPage: true, error: null },
        { source: OTHER, items: [], hasNextPage: false, error: 'Site is down' },
      ]),
    );
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('m2', 'Claymore')], hasNextPage: false }),
    );
    await AppShellWrapper.mount('/search?type=manga&q=dark');
    const view = await screen.findByTestId('manga-search');

    const comics = await within(view).findByRole('region', { name: 'Comic Shelf' });
    expect(within(comics).getByText('Berserk')).toBeInTheDocument();
    const other = within(view).getByRole('region', { name: 'Other Source' });
    expect(within(other).getByRole('alert')).toHaveTextContent('Site is down');

    await user.click(within(comics).getByRole('button', { name: 'More' }));
    expect(await within(view).findByText('Claymore')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', COMICS.id, 'dark', 1, {});

    await user.click(within(view).getByRole('button', { name: 'Filters' }));
    const panel = within(view).getByRole('region', { name: 'Filters' });
    await user.selectOptions(within(panel).getByRole('combobox', { name: 'Status' }), 'completed');
    const tags = within(panel).getByRole('group', { name: 'Tags' });
    await user.click(within(tags).getByRole('button', { name: 'Action: not used' }));
    await user.click(within(tags).getByRole('button', { name: 'Horror: not used' }));
    await user.click(within(tags).getByRole('button', { name: 'Horror: included' }));
    await user.click(within(panel).getByRole('checkbox', { name: 'Has chapters' }));

    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', COMICS.id, 'dark', 1, {
      status: 'completed',
      tags: { include: ['t-action'], exclude: ['t-horror'] },
      hasChapters: true,
    });
    expect(within(view).getByRole('button', { name: '3 filters' })).toBeInTheDocument();
  });

  it("shows a source's popular titles, then browses it with filters alone", async () => {
    const user = userEvent.setup();
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('m3', 'Monster')], hasNextPage: false }),
    );
    await AppShellWrapper.mount('/search?type=manga');
    const view = await screen.findByTestId('manga-search');
    const chips = await within(view).findByRole('group', { name: 'Sources' });
    await user.click(within(chips).getByRole('button', { name: 'Comic Shelf' }));
    expect(within(chips).getByRole('button', { name: 'Comic Shelf' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // Nothing typed: the source's popular titles.
    expect(await within(view).findByRole('heading', { name: 'Popular' })).toBeInTheDocument();
    expect(await within(view).findByText('Monster')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenCalledWith('manga', COMICS.id, '', 1, {});

    await user.click(within(view).getByRole('button', { name: 'Filters' }));
    await user.click(within(view).getByRole('button', { name: 'Horror: not used' }));
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', COMICS.id, '', 1, {
      tags: { include: ['t-horror'], exclude: [] },
    });
  });

  it("shows what's popular before anything is typed", async () => {
    commands.catalogGlobalSearch.mockImplementation(async (medium: string, _query, sources) =>
      ok(
        medium === 'manga' && sources[0] === COMICS.id
          ? [{ source: COMICS, items: [card('m1', 'One Piece')], hasNextPage: true, error: null }]
          : [],
      ),
    );
    commands.podcastTop.mockResolvedValue(
      ok([
        {
          title: 'Top Show',
          author: 'Someone',
          feedUrl: 'https://f.example/top.xml',
          artworkUrl: null,
          description: null,
          subscribed: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/search');
    const all = await screen.findByTestId('all-results');
    const manga = within(all).getByRole('region', { name: 'Comics' });
    expect(await within(manga).findByText('One Piece')).toBeInTheDocument();
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', '', [COMICS.id]);
    const podcasts = within(all).getByRole('region', { name: 'Podcasts' });
    expect(await within(podcasts).findByText('Top Show')).toBeInTheDocument();
    expect(within(all).queryByRole('region', { name: 'In your library' })).not.toBeInTheDocument();
  });

  it('narrows movies and series with pills (M46.25)', async () => {
    const user = userEvent.setup();
    const row = (kind: string, name: string) => ({
      addonId: 'cinemeta',
      addonName: 'Cinemeta',
      kind,
      catalogId: 'top',
      name: 'Popular',
      items: [
        {
          kind,
          id: `tt-${name}`,
          name,
          poster: null,
          posterShape: 'poster',
          releaseInfo: null,
          rating: null,
        },
      ],
      genres: [],
      hasMore: false,
      error: null,
    });
    // Each addon's search catalog is asked on its own.
    const searchCatalog = (kind: string) => ({
      addonId: 'cinemeta',
      addonName: 'Cinemeta',
      kind,
      catalogId: 'top',
      name: 'Popular',
      extras: [],
      board: true,
      searchable: true,
      discoverable: true,
      pages: true,
      failing: false,
    });
    commands.stremioCatalogs.mockResolvedValue(
      ok([searchCatalog('movie'), searchCatalog('series')]),
    );
    commands.stremioSearchCatalog.mockImplementation(async (_addon, kind) =>
      ok(kind === 'movie' ? row('movie', 'Sintel') : row('series', 'Big Buck Series')),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=video&q=b');
    const results = await screen.findByTestId('video-results');
    expect(within(results).getByText('Sintel')).toBeInTheDocument();
    expect(within(results).getByText('Big Buck Series')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Series' }));
    await vi.waitFor(() =>
      expect(screen.getByTestId('video-results')).not.toHaveTextContent('Sintel'),
    );
    expect(screen.getByTestId('video-results')).toHaveTextContent('Big Buck Series');
    // The sources' answers stay, whatever they are.
    expect(screen.getByRole('region', { name: 'From your sources' })).toBeInTheDocument();
  });

  it("puts a video site's own pill in Videos' row, not beside the kinds, and its best answers on Relevant", async () => {
    const user = userEvent.setup();
    const videoSite = {
      id: 'dev.example.videos/videos',
      name: 'Open Videos',
      extensionId: 'dev.example.videos',
      enabled: true,
      searchPill: 'Open Videos',
    };
    const archive = {
      id: 'dev.raygun.ia/films',
      name: 'Internet Archive',
      extensionId: 'dev.raygun.ia',
      enabled: true,
    };
    commands.videoSources.mockResolvedValue(ok([videoSite, archive]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) =>
      ok(
        sources[0] === videoSite.id
          ? [
              {
                source: videoSite,
                items: [card('y1', 'Sintel trailer', videoSite.id)],
                hasNextPage: false,
                error: null,
              },
            ]
          : [
              {
                source: archive,
                items: [card('a1', 'Sintel', archive.id)],
                hasNextPage: false,
                error: null,
              },
            ],
      ),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=video&q=sintel');
    // The main kinds stay as they are; a site's pill sits in Videos' own row.
    const kinds = await screen.findByRole('tablist', { name: 'Search in' });
    expect(
      within(kinds)
        .getAllByRole('tab')
        .map((tab) => tab.textContent),
    ).toEqual(['All', 'Audio', 'Videos', 'Comics', 'Books', 'Your library']);
    const pills = await screen.findByRole('group', { name: 'What to find' });
    await within(pills).findByRole('button', { name: 'Open Videos' });

    // Relevant: the sources' best answers, and a row of the site's with its pill a tap away.
    const sources = await screen.findByRole('region', { name: 'From your sources' });
    expect(await within(sources).findByText('Sintel')).toBeInTheDocument();
    expect(await screen.findByText('Sintel trailer')).toBeInTheDocument();
    const site = screen.getByRole('region', { name: 'Open Videos' });
    await user.click(within(site).getByRole('button', { name: 'More' }));
    expect(within(pills).getByRole('button', { name: 'Open Videos' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('searches the sources chosen with chips, several at once', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources) =>
      ok(
        sources[0] === COMICS.id
          ? [{ source: COMICS, items: [card('m1', 'Berserk')], hasNextPage: true, error: null }]
          : [{ source: OTHER, items: [], hasNextPage: false, error: null }],
      ),
    );
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('m2', 'Claymore')], hasNextPage: false }),
    );
    await AppShellWrapper.mount('/search?type=manga&q=dark');
    const view = await screen.findByTestId('manga-search');
    const chips = await within(view).findByRole('group', { name: 'Sources' });
    expect(within(chips).getByRole('button', { name: 'All sources' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    // Two sources search together (a row each), without filters.
    await user.click(within(chips).getByRole('button', { name: 'Comic Shelf' }));
    await user.click(within(chips).getByRole('button', { name: 'Other Source' }));
    expect(await within(view).findByRole('region', { name: 'Other Source' })).toHaveTextContent(
      'No results',
    );
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', 'dark', [OTHER.id]);
    expect(within(chips).getByRole('button', { name: 'All sources' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    expect(within(view).queryByRole('button', { name: 'Filters' })).not.toBeInTheDocument();

    // Taking one away leaves one source, searched with its filters.
    await user.click(within(chips).getByRole('button', { name: 'Other Source' }));
    expect(await within(view).findByText('Claymore')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', COMICS.id, 'dark', 1, {});

    // All sources again: every source's row, each asked on its own.
    await user.click(within(chips).getByRole('button', { name: 'All sources' }));
    const row = await within(view).findByRole('region', { name: 'Comic Shelf' });
    expect(await within(row).findByText('Berserk')).toBeInTheDocument();
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', 'dark', [COMICS.id]);
  });

  it('finds what you already have', async () => {
    const entry: MangaLibraryEntry = libraryEntry({
      titleId: 't1',
      sourceId: COMICS.id,
      remoteId: 'm1',
      title: 'Frieren',
      unread: 2,
    });
    commands.mangaLibrary.mockResolvedValue(
      ok([entry, { ...entry, titleId: 't2', title: 'Berserk' }]),
    );
    commands.videoHistory.mockResolvedValue(
      ok([
        {
          key: 'url:a',
          title: 'Frieren trailer',
          subtitle: null,
          poster: null,
          positionMs: 0,
          durationMs: null,
          updatedAt: 1,
          watched: true,
        },
      ]),
    );
    await AppShellWrapper.mount('/search?type=library&q=frieren');
    const matches = await screen.findByTestId('library-matches');
    expect(await within(matches).findByText('Frieren')).toBeInTheDocument();
    expect(within(matches).queryByText('Berserk')).not.toBeInTheDocument();
    expect(await within(matches).findByText('Frieren trailer')).toBeInTheDocument();
    expect(commands.librarySearch).toHaveBeenCalledWith('frieren');
  });
});
