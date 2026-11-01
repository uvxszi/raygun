import type { MangaCard, MangaLibraryEntry, SourceFilter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const MANGADEX = {
  id: 'dev.raygun.mangadex/mangadex',
  name: 'MangaDex',
  extensionId: 'dev.raygun.mangadex',
};
const OTHER = {
  id: 'dev.example.other/other',
  name: 'Other Source',
  extensionId: 'dev.example.other',
};
const card = (id: string, title: string, sourceId = MANGADEX.id): MangaCard => ({
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
    commands.mangaSources.mockResolvedValue(ok([MANGADEX, OTHER]));
  });

  it('searches everything at once and leads to each type', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockImplementation(async (medium: string) =>
      ok(
        medium === 'manga'
          ? [{ source: MANGADEX, items: [card('m1', 'Frieren')], hasNextPage: true, error: null }]
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

    const manga = within(all).getByRole('region', { name: 'Manga' });
    expect(await within(manga).findByText('Frieren')).toBeInTheDocument();
    const podcasts = within(all).getByRole('region', { name: 'Podcasts' });
    expect(await within(podcasts).findByText('Frieren Talk')).toBeInTheDocument();
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('manga', 'frieren');
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('novel', 'frieren');

    await user.click(within(manga).getByRole('button', { name: 'See all' }));
    expect(screen.getByRole('tab', { name: 'Manga' })).toHaveAttribute('aria-selected', 'true');
    expect(await screen.findByTestId('manga-search')).toBeInTheDocument();
  });

  it('searches every manga source, then one source with filters and tags', async () => {
    const user = userEvent.setup();
    commands.catalogGlobalSearch.mockResolvedValue(
      ok([
        { source: MANGADEX, items: [card('m1', 'Berserk')], hasNextPage: true, error: null },
        { source: OTHER, items: [], hasNextPage: false, error: 'Site is down' },
      ]),
    );
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('m2', 'Claymore')], hasNextPage: false }),
    );
    await AppShellWrapper.mount('/search?type=manga&q=dark');
    const view = await screen.findByTestId('manga-search');

    const mangadex = await within(view).findByRole('region', { name: 'MangaDex' });
    expect(within(mangadex).getByText('Berserk')).toBeInTheDocument();
    const other = within(view).getByRole('region', { name: 'Other Source' });
    expect(within(other).getByRole('alert')).toHaveTextContent('Site is down');

    await user.click(within(mangadex).getByRole('button', { name: 'More' }));
    expect(await within(view).findByText('Claymore')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', MANGADEX.id, 'dark', 1, {});

    await user.click(within(view).getByRole('button', { name: 'Filters' }));
    const panel = within(view).getByRole('region', { name: 'Filters' });
    await user.selectOptions(within(panel).getByRole('combobox', { name: 'Status' }), 'completed');
    const tags = within(panel).getByRole('group', { name: 'Tags' });
    await user.click(within(tags).getByRole('button', { name: 'Action: not used' }));
    await user.click(within(tags).getByRole('button', { name: 'Horror: not used' }));
    await user.click(within(tags).getByRole('button', { name: 'Horror: included' }));
    await user.click(within(panel).getByRole('checkbox', { name: 'Has chapters' }));

    expect(commands.catalogSearch).toHaveBeenLastCalledWith('manga', MANGADEX.id, 'dark', 1, {
      status: 'completed',
      tags: { include: ['t-action'], exclude: ['t-horror'] },
      hasChapters: true,
    });
    expect(within(view).getByRole('button', { name: '3 filters' })).toBeInTheDocument();
  });

  it('browses a source with filters alone, without typing', async () => {
    const user = userEvent.setup();
    commands.catalogFilters.mockResolvedValue(ok(FILTERS));
    commands.catalogSearch.mockResolvedValue(
      ok({ items: [card('m3', 'Monster')], hasNextPage: false }),
    );
    await AppShellWrapper.mount('/search?type=manga');
    const view = await screen.findByTestId('manga-search');
    await user.selectOptions(
      await within(view).findByRole('combobox', { name: 'Source' }),
      MANGADEX.id,
    );
    expect(
      await within(view).findByText('Type a title, or choose filters to browse.'),
    ).toBeInTheDocument();
    expect(commands.catalogSearch).not.toHaveBeenCalled();

    await user.click(within(view).getByRole('button', { name: 'Filters' }));
    await user.click(within(view).getByRole('button', { name: 'Horror: not used' }));
    expect(await within(view).findByText('Monster')).toBeInTheDocument();
    expect(commands.catalogSearch).toHaveBeenCalledWith('manga', MANGADEX.id, '', 1, {
      tags: { include: ['t-horror'], exclude: [] },
    });
  });

  it('finds what you already have', async () => {
    const entry: MangaLibraryEntry = {
      titleId: 't1',
      sourceId: MANGADEX.id,
      remoteId: 'm1',
      title: 'Frieren',
      coverUrl: null,
      chapters: 10,
      unread: 2,
      genres: [],
      status: 'ongoing',
      addedAt: 0,
      lastReadAt: null,
      latestChapterAt: null,
      started: false,
      downloaded: 0,
      categoryIds: [],
    };
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
