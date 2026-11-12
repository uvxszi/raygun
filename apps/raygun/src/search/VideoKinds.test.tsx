import type {
  Settings,
  StremioAddon,
  StremioCatalogInfo,
  StremioCatalogRow,
  StremioItem,
} from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const addon = (id: string, name: string, adult = false): StremioAddon => ({
  id,
  url: `https://${id}.example/manifest.json`,
  name,
  description: null,
  logo: null,
  version: '1.0.0',
  resources: ['catalog', 'meta'],
  types: ['movie', 'series'],
  catalogs: 2,
  adult,
  error: null,
  enabled: true,
  provides: [],
  p2p: false,
  configureUrl: null,
  needsSetup: false,
  failing: false,
});

const ADDONS = [
  addon('cinemeta', 'Cinemeta'),
  addon('kitsu', 'Kitsu'),
  addon('torrentio', 'Torrentio'),
  addon('late', 'Late Night', true),
];

const searchCatalog = (addonId: string, kind: string): StremioCatalogInfo => ({
  addonId,
  addonName: ADDONS.find((found) => found.id === addonId)!.name,
  kind,
  catalogId: 'search',
  name: 'Search',
  extras: [],
  board: false,
  searchable: true,
  discoverable: false,
  pages: false,
  failing: false,
});

const title = (kind: string, id: string, name: string, releaseInfo: string | null = null) =>
  ({
    kind,
    id,
    name,
    poster: null,
    posterShape: 'poster',
    releaseInfo,
    rating: null,
  }) satisfies StremioItem;

/** What each addon's search catalog of each kind answers. */
const ANSWERS: Record<string, StremioItem[]> = {
  'cinemeta movie': [title('movie', 'tt1', 'Breaking Away', '1979')],
  'cinemeta series': [
    title('series', 'tt0903747', 'Breaking Bad', '2008–2013'),
    title('series', 'tt2', 'Breaking Point', '2020'),
  ],
  // Kitsu's ids are its own: the same show is known by its name and year.
  'kitsu series': [title('series', 'kitsu:1', 'Breaking Bad', '2008')],
  'torrentio series': [
    title('series', 'tt0903747', 'Breaking Bad (2008)', '2008–2013'),
    title('series', 'tt9', 'Bad Breaking', '2022'),
  ],
  'late series': [title('series', 'late:1', 'Breaking Bad Habits', '2021')],
};

const CATALOGS = Object.keys(ANSWERS).map((key) => {
  const [addonId, kind] = key.split(' ') as [string, string];
  return searchCatalog(addonId, kind);
});

const answer = (addonId: string, kind: string): StremioCatalogRow => ({
  addonId,
  addonName: ADDONS.find((found) => found.id === addonId)!.name,
  kind,
  catalogId: 'search',
  name: 'Search',
  items: ANSWERS[`${addonId} ${kind}`] ?? [],
  genres: [],
  hasMore: false,
  error: null,
});

/** Settings kept by the mocked commands, starting from `start`. */
const keepSettings = (start: Partial<Settings>) => {
  let stored: Settings = { ...DEFAULT_SETTINGS, ...start };
  commands.settingsGet.mockImplementation(async () => ok(stored));
  commands.settingsUpdate.mockImplementation(async (change: { key: string; value: unknown }) => {
    stored = { ...stored, [change.key]: change.value };
    return ok(stored);
  });
};

/** The titles a section's cards name, in order. */
const cardNames = (section: HTMLElement) =>
  within(section)
    .getAllByTestId('stremio-card')
    .map((card) => card.getAttribute('aria-label'));

const searchFor = async (query: string, path = '/browse?tab=search&type=video') => {
  await AppShellWrapper.mount(`${path}&q=${encodeURIComponent(query)}`);
  const results = await screen.findByTestId('video-results');
  // Every addon answered.
  await waitFor(() => expect(within(results).queryByText('Searching…')).not.toBeInTheDocument());
  return results;
};

describe('Videos search: a section per kind', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    keepSettings({ showNsfw: true });
    commands.videoSources.mockResolvedValue(ok([]));
    commands.stremioAddons.mockResolvedValue(ok(ADDONS));
    commands.stremioCatalogs.mockResolvedValue(ok(CATALOGS));
    commands.stremioSearchCatalog.mockImplementation(async (addonId: string, kind: string) =>
      ok(answer(addonId, kind)),
    );
  });

  it("merges every addon's answers into one section per kind, ranked, each title once", async () => {
    const user = userEvent.setup();
    const results = await searchFor('breaking bad');

    expect(within(results).getAllByRole('region', { name: 'Series' })).toHaveLength(1);
    expect(within(results).getAllByRole('region', { name: 'Movies' })).toHaveLength(1);
    const series = within(results).getByRole('region', { name: 'Series' });
    // Every addon's titles ranked as one list; Breaking Bad once for three addons.
    expect(cardNames(series)).toEqual([
      'Breaking Bad',
      'Breaking Bad Habits',
      'Bad Breaking',
      'Breaking Point',
    ]);
    expect(within(series).getByRole('link', { name: 'Breaking Bad' })).toHaveAttribute(
      'href',
      expect.stringContaining('id=tt0903747'),
    );

    // A title from one addon names it; from an addon rated for adults, marked so.
    const [, habits, bad, point] = within(series).getAllByTestId('stremio-card');
    expect(within(point!).getByTestId('source-tag')).toHaveTextContent('Cinemeta');
    expect(within(bad!).getByTestId('source-tag')).toHaveTextContent('Torrentio');
    expect(within(habits!).getByTestId('source-tag')).toHaveTextContent('Late Night');
    expect(within(habits!).getByTestId('adult-mark')).toBeInTheDocument();
    expect(within(point!).queryByTestId('adult-mark')).not.toBeInTheDocument();

    // Several: how many, and each one's own copy a tap away.
    const sources = within(series).getByRole('button', { name: 'Breaking Bad: 3 sources' });
    expect(sources).toHaveTextContent('3 sources');
    await user.click(sources);
    expect(screen.getByRole('menuitem', { name: 'Cinemeta' })).toBeInTheDocument();
    // A copy named its own way says so.
    expect(
      screen.getByRole('menuitem', { name: 'Torrentio · Breaking Bad (2008)' }),
    ).toBeInTheDocument();
    commands.stremioMeta.mockClear();
    await user.click(screen.getByRole('menuitem', { name: 'Kitsu' }));
    await waitFor(() => expect(commands.stremioMeta).toHaveBeenCalledWith('series', 'kitsu:1'));
  });

  it('shows a row per addon By source, as each answered, and remembers it', async () => {
    const user = userEvent.setup();
    const results = await searchFor('breaking bad');
    const arrange = within(results).getByRole('group', { name: 'Arrange results' });
    expect(within(arrange).getByRole('button', { name: 'Best matches' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    await user.click(within(arrange).getByRole('button', { name: 'By source' }));
    await waitFor(() =>
      expect(within(results).getAllByRole('region', { name: 'Series' })).toHaveLength(4),
    );
    const rows = within(results).getAllByRole('region', { name: 'Series' });
    expect(rows.map((row) => within(row).getByTestId('source-tag').textContent)).toEqual([
      'Cinemeta',
      'Kitsu',
      'Torrentio',
      'Late Night',
    ]);
    expect(cardNames(rows[2]!)).toEqual(['Breaking Bad (2008)', 'Bad Breaking']);
    expect(within(results).queryByTestId('title-sources')).not.toBeInTheDocument();
    expect(localStorage.getItem('raygun.search.grouped.video')).toBe('1');

    await user.click(within(arrange).getByRole('button', { name: 'Best matches' }));
    await waitFor(() =>
      expect(within(results).getAllByRole('region', { name: 'Series' })).toHaveLength(1),
    );
  });

  it('previews the first kinds on All, without the switch or See all', async () => {
    const many = Array.from({ length: 30 }, (_, index) =>
      title('series', `tt-many-${index}`, `Breaking Story ${index + 21}`, '2001'),
    );
    commands.stremioSearchCatalog.mockImplementation(async (addonId: string, kind: string) =>
      ok(
        addonId === 'kitsu'
          ? { ...answer(addonId, kind), items: [...answer(addonId, kind).items, ...many] }
          : answer(addonId, kind),
      ),
    );
    await AppShellWrapper.mount('/browse?tab=search&q=breaking%20bad');
    const videos = await screen.findByRole('region', { name: 'Videos' });
    const results = await within(videos).findByTestId('video-results');
    await waitFor(() => expect(within(results).queryByText('Searching…')).not.toBeInTheDocument());
    expect(
      within(results)
        .getAllByTestId('video-kind')
        .map((section) => section.getAttribute('aria-label')),
    ).toEqual(['Movies', 'Series']);
    expect(within(results).queryByRole('group', { name: 'Arrange results' })).toBeNull();
    expect(within(results).queryByRole('button', { name: /See all of/ })).toBeNull();
  });

  it('follows 18+ only and Playable only in the merged sections', async () => {
    const user = userEvent.setup();
    const results = await searchFor('breaking bad');
    await user.click(screen.getByRole('button', { name: /^Search filters/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Search filters' });
    await user.click(within(sheet).getByRole('switch', { name: '18+ only' }));
    await waitFor(() =>
      expect(cardNames(within(results).getByRole('region', { name: 'Series' }))).toEqual([
        'Breaking Bad Habits',
      ]),
    );
    expect(within(results).queryByRole('region', { name: 'Movies' })).not.toBeInTheDocument();
  });

  it("opens Series' pill from See all, with every series in a grid", async () => {
    const user = userEvent.setup();
    const many = Array.from({ length: 30 }, (_, index) =>
      title('series', `tt-many-${index}`, `Breaking News ${index + 1}`, '2001'),
    );
    commands.stremioSearchCatalog.mockImplementation(async (addonId: string, kind: string) =>
      ok(
        addonId === 'torrentio' && kind === 'series'
          ? { ...answer(addonId, kind), items: many }
          : answer(addonId, kind),
      ),
    );
    const results = await searchFor('breaking news');
    const series = within(results).getByRole('region', { name: 'Series' });
    // A row shows the best twenty.
    await waitFor(() => expect(within(series).getAllByTestId('stremio-card')).toHaveLength(20));

    await user.click(within(series).getByRole('button', { name: 'See all of Series' }));
    expect(screen.getByRole('button', { name: 'Series' })).toHaveAttribute('aria-pressed', 'true');
    // Thirty from one addon, Breaking Bad from two, two more.
    await waitFor(() => {
      const pill = screen.getByTestId('video-results');
      expect(
        within(within(pill).getByRole('region', { name: 'Series' })).getAllByTestId('stremio-card'),
      ).toHaveLength(33);
    });
    expect(
      within(screen.getByTestId('video-results')).queryByRole('region', { name: 'Movies' }),
    ).not.toBeInTheDocument();
  });

  it('opens a kind without a pill of its own in place', async () => {
    const user = userEvent.setup();
    const channels = Array.from({ length: 25 }, (_, index) =>
      title('channel', `yt:${index}`, `Breaking Channel ${index + 1}`),
    );
    commands.stremioCatalogs.mockResolvedValue(
      ok([...CATALOGS, searchCatalog('cinemeta', 'channel')]),
    );
    commands.stremioSearchCatalog.mockImplementation(async (addonId: string, kind: string) =>
      ok(
        kind === 'channel' ? { ...answer(addonId, kind), items: channels } : answer(addonId, kind),
      ),
    );
    const results = await searchFor('breaking');
    const row = within(results).getByRole('region', { name: 'Channels' });
    await waitFor(() => expect(within(row).getAllByTestId('stremio-card')).toHaveLength(20));
    // Few enough Movies: nothing more to see there.
    expect(
      within(within(results).getByRole('region', { name: 'Movies' })).queryByRole('button', {
        name: /See all/,
      }),
    ).not.toBeInTheDocument();

    await user.click(within(row).getByRole('button', { name: 'See all of Channels' }));
    await waitFor(() =>
      expect(
        within(within(results).getByRole('region', { name: 'Channels' })).getAllByTestId(
          'stremio-card',
        ),
      ).toHaveLength(25),
    );
    expect(screen.getByRole('button', { name: 'Relevant' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});
