import type {
  MusicChart,
  OnlineTrack,
  SavedTrack,
  SettingChange,
  Settings,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';
import { sourceExtension } from '../test/sourceExtension';
import { pickChart } from './useDiscovery';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const provider = (extensionId: string, name: string) => ({
  id: `${extensionId}/charts`,
  extensionId,
  kind: 'music.charts',
  name,
});

const chart = (
  extensionId: string,
  name: string,
  id: string,
  extra: Partial<MusicChart> = {},
): MusicChart => ({
  provider: provider(extensionId, name.split(':')[0] ?? name),
  id,
  name,
  description: null,
  artworkUrl: null,
  tag: null,
  group: extra.tag ? 'genre' : 'top',
  ...extra,
});

const CHARTS = [
  chart('dev.example.charts', 'Top songs: GB', 'most-played'),
  chart('dev.example.listens', 'Most listened this week', 'week', { group: 'trending' }),
  chart('dev.example.tunes', 'Rock', '152', { tag: 'Rock', artworkUrl: 'https://e.x/rock.jpg' }),
];

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
});

const saved = (title: string, artist: string): SavedTrack => ({
  trackKey: `${artist.toLowerCase()}|${title.toLowerCase()}`,
  title,
  artist,
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
  local: false,
  addedAt: 1,
  playCount: 0,
});

const withCharts = () => {
  commands.musicCharts.mockResolvedValue(ok({ charts: CHARTS, errors: [] }));
  commands.musicChart.mockImplementation(async (_provider: string, id: string) =>
    ok(
      id === 'most-played'
        ? [song('Babydoll', 'Jamie Miller'), song('Housewife', 'Tate McRae')]
        : id === 'week'
          ? [song('SWIM', 'BTS')]
          : [song('Thunderstruck', 'AC/DC')],
    ),
  );
  commands.musicTagVocabulary.mockResolvedValue([
    { name: 'Chill', kind: 'mood' },
    { name: 'Rock', kind: 'genre' },
    { name: 'Breakcore', kind: 'genre' },
    { name: 'Polka', kind: 'genre' },
  ]);
};

describe('Music discovery', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withCharts();
  });

  it("picks Home's charts by their kind, whichever source has them", () => {
    expect(pickChart(CHARTS, 'popular')?.id).toBe('most-played');
    expect(pickChart(CHARTS, 'trending')?.id).toBe('week');
    expect(pickChart([CHARTS[2] as MusicChart], 'popular')).toBeNull();
    expect(pickChart([], 'trending')).toBeNull();
  });

  it('points Browse all to the extensions without a chart source', async () => {
    commands.musicCharts.mockResolvedValue(ok({ charts: [], errors: [] }));
    await AppShellWrapper.mount('/browse');
    const all = await screen.findByTestId('browse-all');
    await vi.waitFor(() =>
      expect(within(all).getByTestId('no-sources')).toHaveTextContent('No chart sources yet.'),
    );
    expect(within(all).getByRole('link', { name: 'Find extensions' })).toBeInTheDocument();
    expect(
      within(all)
        .getAllByTestId('kind-tile')
        .map((tile) => tile.textContent),
    ).not.toContain('Audio');
  });

  it('leaves the chart rows out of Home without a chart source', async () => {
    commands.musicCharts.mockResolvedValue(ok({ charts: [], errors: [] }));
    await AppShellWrapper.mount('/');
    await screen.findByTestId('home-page');
    await vi.waitFor(() => expect(commands.musicCharts).toHaveBeenCalled());
    expect(screen.queryByRole('region', { name: 'Popular now' })).not.toBeInTheDocument();
    expect(screen.queryByRole('region', { name: 'Trending' })).not.toBeInTheDocument();
    expect(screen.queryByTestId('source-issue')).not.toBeInTheDocument();
  });

  it('shows Popular now and Trending on Home, and plays from a chart', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const popular = await screen.findByRole('region', { name: 'Popular now' });
    const trending = await screen.findByRole('region', { name: 'Trending' });
    expect(await within(trending).findByText('SWIM')).toBeInTheDocument();
    await user.click(await within(popular).findByRole('button', { name: 'Play Housewife' }));
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      [
        expect.objectContaining({ title: 'Babydoll' }),
        expect.objectContaining({ title: 'Housewife' }),
      ],
      1,
    );
  });

  it("lists everything in Browse all, Spotify's way", async () => {
    const user = userEvent.setup();
    commands.extensionsList.mockResolvedValue(ok([sourceExtension(['music.charts'])]));
    commands.musicTagSummary.mockResolvedValue(
      ok([{ name: 'Breakcore', kind: 'genre', count: 4 }]),
    );
    await AppShellWrapper.mount('/browse');
    const all = await screen.findByTestId('browse-all');
    expect(
      within(all)
        .getAllByTestId('kind-tile')
        .map((tile) => tile.textContent),
    ).toEqual([
      'Made for You',
      'Audio',
      'Audiobooks',
      'Podcasts',
      'Comics',
      'Books',
      'Videos',
      'Websites',
    ]);
    const charts = await within(all).findByRole('region', { name: 'Charts' });
    expect(
      within(charts)
        .getAllByTestId('chart-tile')
        .map((tile) => tile.textContent),
    ).toEqual(['Top songs: GB', 'Most listened this week']);
    expect(
      within(await within(all).findByRole('region', { name: 'In your songs' })).getByText(
        'Breakcore',
      ),
    ).toBeInTheDocument();
    const genres = within(all).getByRole('region', { name: 'Genres & moods' });
    expect(within(genres).getByText('Chill')).toBeInTheDocument();
    expect(within(genres).queryByText('Polka')).not.toBeInTheDocument();
    await user.click(within(all).getByRole('button', { name: 'Show all' }));
    expect(within(genres).getByText('Polka')).toBeInTheDocument();

    await user.click(within(charts).getByText('Top songs: GB'));
    const page = await screen.findByTestId('music-chart-page');
    expect(within(page).getByRole('heading', { level: 1 })).toHaveTextContent('Top songs: GB');
    expect(await within(page).findAllByTestId('online-track-row')).toHaveLength(2);
    await user.click(within(page).getByRole('button', { name: 'Play all' }));
    expect(commands.playbackReplace).toHaveBeenLastCalledWith(expect.any(Array), 0);
  });

  it("opens a genre's page with its charts and the user's songs", async () => {
    commands.musicTagSongs.mockResolvedValue(ok([saved('Back in Black', 'AC/DC')]));
    await AppShellWrapper.mount('/music/tag?name=Rock');
    const page = await screen.findByTestId('music-tag-page');
    expect(within(page).getByRole('heading', { level: 1 })).toHaveTextContent('Rock');
    expect(await within(page).findByText('Thunderstruck')).toBeInTheDocument();
    expect(await within(page).findByText('Back in Black')).toBeInTheDocument();
    expect(commands.musicTagSongs).toHaveBeenCalledWith('Rock');
  });

  it("fills a mood's page with every source's songs, and calmly says which failed", async () => {
    const user = userEvent.setup();
    commands.musicTagSources.mockResolvedValue(
      ok({
        rows: [
          {
            provider: provider('dev.example.tunes', 'Tunes'),
            name: 'Chill radio on Tunes',
            tracks: [song('Sunset Lover', 'Petit Biscuit'), song('Weightless', 'Marconi Union')],
          },
          {
            provider: provider('dev.example.listens', 'Listens'),
            name: 'Chill on Listens',
            tracks: [song('Teardrop', 'Massive Attack')],
          },
        ],
        failures: [{ source: 'Indie Tracks', error: 'Indie Tracks answered HTTP 503' }],
      }),
    );
    await AppShellWrapper.mount('/music/tag?name=Chill');
    const page = await screen.findByTestId('music-tag-page');
    const rows = await within(page).findAllByTestId('tag-source-row');
    expect(rows.map((row) => within(row).getByRole('heading').textContent)).toEqual([
      'Chill radio on Tunes',
      'Chill on Listens',
    ]);
    expect(commands.musicTagSources).toHaveBeenCalledWith('Chill', expect.anything());
    expect(within(page).getByTestId('source-issue')).toHaveTextContent(
      'Indie Tracks is having problems — try later',
    );

    await user.click(
      within(rows[0] as HTMLElement).getByRole('button', { name: 'Play Weightless' }),
    );
    expect(commands.playbackReplace).toHaveBeenLastCalledWith(
      [
        expect.objectContaining({ title: 'Sunset Lover' }),
        expect.objectContaining({ title: 'Weightless' }),
      ],
      1,
    );
  });

  it('groups charts by kind in Browse → Music', async () => {
    commands.musicCharts.mockResolvedValue(
      ok({
        charts: [
          ...CHARTS,
          chart('dev.example.indie', 'Underground trending', 'underground', {
            group: 'underground',
          }),
          chart('dev.example.listens', 'Fresh singles', 'fresh', { group: 'new' }),
          chart('dev.example.tunes', "The '80s", 'radio:38305', { group: 'decade' }),
        ],
        errors: [],
      }),
    );
    await AppShellWrapper.mount('/music/charts');
    const page = await screen.findByTestId('music-charts-page');
    const headings = await within(page).findAllByRole('heading', { level: 2 });
    expect(
      headings
        .map((heading) => heading.textContent)
        .filter((text) =>
          ['Top charts', 'New releases', 'Underground', 'Decades', 'Genres'].includes(text ?? ''),
        ),
    ).toEqual(['Top charts', 'New releases', 'Underground', 'Decades', 'Genres']);
  });

  it("filters a song table by its songs' tags and shows them in a column", async () => {
    const user = userEvent.setup();
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: true,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    commands.favoritesList.mockResolvedValue(
      ok([saved('Get Lucky', 'Daft Punk'), saved('Thunderstruck', 'AC/DC')]),
    );
    commands.musicTagsFor.mockResolvedValue(
      ok([
        {
          trackKey: 'daft punk|get lucky',
          tags: [
            { name: 'Funk', kind: 'genre' },
            { name: 'Happy', kind: 'mood' },
          ],
        },
        { trackKey: 'ac/dc|thunderstruck', tags: [{ name: 'Rock', kind: 'genre' }] },
      ]),
    );
    await AppShellWrapper.mount('/library?view=favorites');
    const chips = await screen.findByRole('group', { name: 'Filter by genre or mood' });
    await user.click(within(chips).getByRole('button', { name: 'Rock' }));
    expect(screen.getAllByTestId('track-table-row')).toHaveLength(1);
    expect(screen.getByTestId('track-table-row')).toHaveTextContent('Thunderstruck');
    await user.click(within(chips).getByRole('button', { name: 'All' }));
    expect(screen.getAllByTestId('track-table-row')).toHaveLength(2);
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('sets the charts country and background tagging in Settings', async () => {
    const user = userEvent.setup();
    let current: Settings = DEFAULT_SETTINGS;
    commands.settingsGet.mockImplementation(async () => ok(current));
    commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
      current = { ...current, [change.key]: change.value };
      return ok(current);
    });
    await AppShellWrapper.mount('/settings/browse');
    await user.click(await screen.findByRole('button', { name: /Charts country/ }));
    await user.click(await screen.findByRole('radio', { name: 'Japan' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'discovery',
      value: { region: 'jp', tagSongs: true },
    });
    await user.click(screen.getByRole('switch', { name: /Find genres and moods/ }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'discovery',
      value: { region: 'jp', tagSongs: false },
    });
  });
});
