import type {
  MadeForYou,
  MusicChart,
  OnlineTrack,
  PodcastEpisode,
  SettingChange,
  Settings,
} from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chart = (extensionId: string, id: string, name: string): MusicChart => ({
  provider: { id: `${extensionId}/charts`, extensionId, kind: 'music.charts', name: 'Charts' },
  id,
  name,
  description: null,
  artworkUrl: null,
  tag: null,
  group: 'top',
});

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  explicit: false,
});

const withCharts = (songs: Record<string, OnlineTrack[]>) => {
  commands.musicCharts.mockResolvedValue(
    ok({
      charts: [
        chart('dev.raygun.apple-charts', 'most-played', 'Top songs'),
        chart('dev.raygun.listenbrainz', 'week', 'This week'),
      ],
      errors: [],
    }),
  );
  commands.musicChart.mockImplementation(async (_provider: string, id: string) =>
    ok(songs[id] ?? []),
  );
};

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const forYou = (topArtists: string[]): MadeForYou => ({
  mixes: [],
  stats: {
    playsThisWeek: 0,
    minutesThisWeek: 0,
    topArtist: null,
    topSong: null,
    peakHour: null,
    songsKnown: 3,
    topArtists,
  },
  madeAt: 1,
  making: false,
});

const episode = (id: string, title: string, played = false): PodcastEpisode => ({
  id,
  podcastId: 'show-1',
  podcastTitle: 'The Show',
  title,
  description: null,
  publishedAt: 1_800_000_000_000,
  durationMs: 1_800_000,
  positionMs: null,
  played,
  artworkUrl: null,
  video: false,
  download: null,
  number: null,
});

describe('Home', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it("hides an artist's songs from Home at once, says so, and Undo brings them back", async () => {
    const user = userEvent.setup();
    withCharts({
      'most-played': [song('Babydoll', 'Jamie Miller'), song('Housewife', 'Tate McRae')],
      week: [song('Sports car', 'Tate McRae'), song('SWIM', 'BTS')],
    });
    await AppShellWrapper.mount('/');
    const popular = await screen.findByRole('region', { name: 'Popular now' });
    const trending = await screen.findByRole('region', { name: 'Trending' });
    const card = await within(popular).findByRole('button', { name: 'Play Housewife' });

    fireEvent.contextMenu(card, { clientX: 20, clientY: 20 });
    const menu = await screen.findByRole('menu');
    await user.click(within(menu).getByRole('menuitem', { name: "Don't show Tate McRae" }));

    expect(commands.hiddenArtistSet).toHaveBeenCalledWith('Tate McRae', true);
    // Gone from every Home row, not only the one it was hidden from.
    await vi.waitFor(() =>
      expect(within(popular).queryByRole('button', { name: 'Play Housewife' })).toBeNull(),
    );
    expect(within(trending).queryByText('Sports car')).toBeNull();
    expect(within(trending).getByText('SWIM')).toBeInTheDocument();
    const toast = await screen.findByTestId('toast');
    expect(toast).toHaveTextContent("Tate McRae won't be shown or suggested");

    await user.click(within(toast).getByRole('button', { name: 'Undo' }));
    expect(commands.hiddenArtistSet).toHaveBeenLastCalledWith('Tate McRae', false);
    expect(await within(popular).findByRole('button', { name: 'Play Housewife' })).toBeVisible();
  });

  it('marks a song Not interested, and offers it only in Home rows', async () => {
    const user = userEvent.setup();
    withCharts({
      'most-played': [song('Babydoll', 'Jamie Miller'), song('Housewife', 'Tate McRae')],
    });
    await AppShellWrapper.mount('/');
    const popular = await screen.findByRole('region', { name: 'Popular now' });
    fireEvent.contextMenu(await within(popular).findByRole('button', { name: 'Play Babydoll' }), {
      clientX: 20,
      clientY: 20,
    });
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Not interested' }),
    );
    expect(commands.hiddenItemSet).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'song', key: 'jamie miller|babydoll', name: 'Babydoll' }),
      true,
    );
    await vi.waitFor(() =>
      expect(within(popular).queryByRole('button', { name: 'Play Babydoll' })).toBeNull(),
    );
    expect(within(popular).getByRole('button', { name: 'Play Housewife' })).toBeInTheDocument();
    expect(await screen.findByTestId('toast')).toHaveTextContent("Babydoll won't show on Home");
  });

  it('hides a row from its ⋯ menu, with Undo, and leads to Edit Home', async () => {
    const user = userEvent.setup();
    withSettings(DEFAULT_SETTINGS);
    withCharts({ 'most-played': [song('Babydoll', 'Jamie Miller')], week: [song('SWIM', 'BTS')] });
    await AppShellWrapper.mount('/');
    const trending = await screen.findByRole('region', { name: 'Trending' });
    await user.click(within(trending).getByRole('button', { name: 'More for Trending' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Hide this row' }),
    );

    await vi.waitFor(() =>
      expect(screen.queryByRole('region', { name: 'Trending' })).not.toBeInTheDocument(),
    );
    const saved = commands.settingsUpdate.mock.calls.at(-1)?.[0] as {
      value: { homeSections: { id: string; visible: boolean }[] };
    };
    expect(saved.value.homeSections.find((item) => item.id === 'trendingMusic')?.visible).toBe(
      false,
    );
    const toast = await screen.findByTestId('toast');
    expect(toast).toHaveTextContent('Trending is hidden from Home');
    await user.click(within(toast).getByRole('button', { name: 'Undo' }));
    expect(await screen.findByRole('region', { name: 'Trending' })).toBeInTheDocument();

    // A right click on the heading offers the same, and Edit Home.
    fireEvent.contextMenu(
      within(screen.getByRole('region', { name: 'Popular now' })).getByRole('heading'),
      {
        clientX: 20,
        clientY: 20,
      },
    );
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Edit Home' }),
    );
    expect(await screen.findByTestId('layout-editor')).toBeInTheDocument();
  });

  it('leaves out rows with nothing in them, and leads each to all of it', async () => {
    withCharts({ 'most-played': [song('Babydoll', 'Jamie Miller')], week: [] });
    await AppShellWrapper.mount('/');
    const popular = await screen.findByRole('region', { name: 'Popular now' });
    expect(await within(popular).findByRole('button', { name: 'Play Babydoll' })).toBeVisible();
    expect(within(popular).getByRole('link', { name: 'Show all of Popular now' })).toHaveAttribute(
      'href',
      '/music/chart?provider=dev.raygun.apple-charts%2Fcharts&id=most-played',
    );
    await vi.waitFor(() =>
      expect(screen.queryByRole('region', { name: 'Trending' })).not.toBeInTheDocument(),
    );
  });

  it('shows top artists, new episodes, followed releases and popular podcasts', async () => {
    commands.madeForYou.mockResolvedValue(ok(forYou(['Nina Simone', 'Hidden Band', 'Etta James'])));
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        hidden: { artists: [{ name: 'hidden band', hiddenAt: 1 }], items: [] },
      }),
    );
    commands.podcastLatest.mockResolvedValue(
      ok([episode('e1', 'Fresh one'), episode('e2', 'Heard already', true)]),
    );
    commands.artistReleaseNews.mockResolvedValue(
      ok([
        {
          artist: 'Nina Simone',
          album: {
            provider: { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' },
            id: 'a1',
            title: 'New Album',
            artist: 'Nina Simone',
            year: 2026,
            artworkUrl: null,
            trackCount: 10,
          },
          foundAt: 1,
        },
      ]),
    );
    commands.podcastTop.mockResolvedValue(
      ok([
        {
          title: 'Top Show',
          author: 'Someone',
          feedUrl: 'https://feeds.example/top.xml',
          artworkUrl: null,
          subscribed: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/');

    const artists = await screen.findByRole('region', { name: 'Your top artists' });
    expect(within(artists).getByRole('link', { name: /Nina Simone/ })).toHaveAttribute(
      'href',
      '/artist?name=Nina+Simone',
    );
    expect(within(artists).queryByText('Hidden Band')).not.toBeInTheDocument();

    const episodes = await screen.findByRole('region', { name: 'New episodes' });
    expect(within(episodes).getByText('Fresh one')).toBeInTheDocument();
    expect(within(episodes).queryByText('Heard already')).not.toBeInTheDocument();
    await userEvent.click(
      within(episodes).getByRole('button', { name: 'Play Fresh one from The Show' }),
    );
    expect(commands.podcastPlay).toHaveBeenCalledWith('e1');

    const releases = await screen.findByRole('region', { name: 'New from artists you follow' });
    expect(within(releases).getByText('New Album')).toBeInTheDocument();
    const podcasts = await screen.findByRole('region', { name: 'Popular podcasts' });
    expect(within(podcasts).getByText('Top Show')).toBeInTheDocument();
  });

  it('starts new listeners off with plain chips, not a paragraph', async () => {
    await AppShellWrapper.mount('/');
    const start = await screen.findByRole('region', { name: 'Get started' });
    expect(
      within(start)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['Search', 'Browse all', 'Podcasts', 'Audiobooks', 'Library']);
    expect(start.querySelector('p')).toBeNull();
  });
});
