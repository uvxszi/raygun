import type {
  StremioAddon,
  StremioCatalogRow,
  StremioMeta,
  StremioStream,
  StremioVideo,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ROW: StremioCatalogRow = {
  addonId: 'cinemeta',
  addonName: 'Cinemeta',
  kind: 'series',
  catalogId: 'top',
  name: 'Popular',
  items: [
    {
      kind: 'series',
      id: 'tt0903747',
      name: 'Breaking Bad',
      poster: null,
      posterShape: 'poster',
      releaseInfo: '2008–2013',
      rating: '9.5',
    },
  ],
  genres: ['Drama', 'Crime'],
  hasMore: true,
  error: null,
};

const episode = (season: number, number: number, extra: Partial<StremioVideo> = {}) => ({
  id: `tt0903747:${season}:${number}`,
  title: `Episode ${number}`,
  season,
  episode: number,
  released: '2008-01-20T00:00:00.000Z',
  thumbnail: null,
  overview: null,
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  ...extra,
});

const SERIES: StremioMeta = {
  kind: 'series',
  id: 'tt0903747',
  name: 'Breaking Bad',
  poster: null,
  background: null,
  logo: null,
  description: 'A chemistry teacher turns to crime.',
  genres: ['Drama'],
  releaseInfo: '2008–2013',
  runtime: '49 min',
  rating: '9.5',
  cast: ['Bryan Cranston'],
  director: [],
  trailerYtIds: [],
  videos: [episode(1, 1, { watched: true }), episode(1, 2), episode(2, 1)],
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  addonName: 'Cinemeta',
  inLibrary: false,
  titleId: null,
};

const MOVIE: StremioMeta = {
  ...SERIES,
  kind: 'movie',
  id: 'tt0133093',
  name: 'The Matrix',
  releaseInfo: '1999',
  videos: [],
  trailerYtIds: ['vKQi3bBA1y8'],
};

const stream = (kind: StremioStream['kind'], name: string, target: string): StremioStream => ({
  kind,
  name,
  description: null,
  target,
  headers: {},
  subtitles: [],
  bingeGroup: null,
});

const ADDON: StremioAddon = {
  id: 'cinemeta',
  url: 'https://v3-cinemeta.strem.io/manifest.json',
  name: 'Cinemeta',
  description: 'Movies and series',
  logo: null,
  version: '3.0.13',
  resources: ['catalog', 'meta'],
  types: ['movie', 'series'],
  catalogs: 4,
  adult: false,
  error: null,
};

describe('Stremio addons in Watch', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows catalog rows, opens a series and plays an episode stream', async () => {
    const user = userEvent.setup();
    commands.stremioHome.mockResolvedValue(ok([ROW]));
    commands.stremioMeta.mockResolvedValue(ok(SERIES));
    commands.stremioStreams.mockResolvedValue(
      ok([
        {
          addonId: 'local',
          addonName: 'Local streams',
          streams: [
            stream('torrent', 'Torrent 1080p', 'abc'),
            stream('direct', 'HLS 720p', 'https://cdn.example/e2.m3u8'),
          ],
          error: null,
        },
      ]),
    );
    commands.stremioAddons.mockResolvedValue(ok([ADDON]));
    await AppShellWrapper.mount('/');
    // Addons with catalogues are sources in Browse; each opens its catalogue rows.
    await user.click(screen.getByTestId('nav-browse-tab'));
    await user.click(await screen.findByRole('link', { name: 'Cinemeta' }));
    await screen.findByTestId('stremio-addon-page');

    const row = await screen.findByRole('region', { name: 'Popular · Series' });
    await user.click(within(row).getByRole('link', { name: 'Breaking Bad' }));
    const page = await screen.findByTestId('watch-title-page');
    expect(commands.stremioMeta).toHaveBeenCalledWith('series', 'tt0903747');
    expect(within(page).getByText('2008–2013 · 49 min · ★ 9.5')).toBeInTheDocument();

    // Season 1 opens first: it has an unwatched episode.
    expect(within(page).getByRole('tab', { name: 'Season 1' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(
      within(page).getByRole('button', { name: 'Mark S1 E1 · Episode 1 unwatched' }),
    ).toHaveAttribute('aria-pressed', 'true');

    const second = within(page).getByRole('listitem', { name: 'S1 E2 · Episode 2' });
    // A tap watches: the player opens and finds the best stream itself (M44).
    await user.click(within(second).getByRole('button', { name: /^2\. Episode 2/ }));
    expect(commands.stremioWatch).toHaveBeenCalledWith('series', 'tt0903747', 'tt0903747:1:2');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    // A long press picks the stream.
    await user.pointer({
      keys: '[MouseRight]',
      target: within(second).getByRole('button', { name: /^2\. Episode 2/ }),
    });
    const picker = await screen.findByRole('dialog', { name: 'Streams' });
    expect(commands.stremioStreams).toHaveBeenCalledWith('series', 'tt0903747:1:2');
    expect(within(picker).getByRole('button', { name: /Torrent 1080p/ })).toBeDisabled();
    expect(within(picker).getByText("Torrent streams aren't supported")).toBeInTheDocument();

    await user.click(within(picker).getByRole('button', { name: /^HLS 720p/ }));
    expect(commands.stremioPlay).toHaveBeenCalledWith(
      {
        kind: 'series',
        metaId: 'tt0903747',
        videoId: 'tt0903747:1:2',
        title: 'Breaking Bad',
        subtitle: 'S1 E2 · Episode 2',
        poster: null,
        stream: stream('direct', 'HLS 720p', 'https://cdn.example/e2.m3u8'),
        external: false,
      },
      'en',
    );
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    await user.click(within(page).getByRole('tab', { name: 'Season 2' }));
    await user.click(within(page).getByRole('button', { name: 'Mark S2 E1 · Episode 1 watched' }));
    expect(commands.stremioSetWatched).toHaveBeenCalledWith(
      'series',
      'tt0903747:2:1',
      'Breaking Bad',
      'S2 E1 · Episode 1',
      null,
      true,
    );
  });

  it('plays a movie, its trailer, and says when no addon has streams', async () => {
    const user = userEvent.setup();
    commands.stremioMeta.mockResolvedValue(ok(MOVIE));
    await AppShellWrapper.mount('/watch/title?type=movie&id=tt0133093');
    const page = await screen.findByTestId('watch-title-page');

    await user.click(within(page).getByRole('button', { name: 'Trailer' }));
    expect(commands.stremioPlay).toHaveBeenCalledWith(
      expect.objectContaining({
        videoId: 'tt0133093',
        stream: expect.objectContaining({ kind: 'youTube', target: 'vKQi3bBA1y8' }),
      }),
      'en',
    );

    await user.click(within(page).getByRole('button', { name: 'Mark watched' }));
    expect(commands.stremioSetWatched).toHaveBeenCalledWith(
      'movie',
      'tt0133093',
      'The Matrix',
      '1999',
      null,
      true,
    );

    await user.click(within(page).getByRole('button', { name: 'Play' }));
    expect(commands.stremioWatch).toHaveBeenCalledWith('movie', 'tt0133093', null);

    await user.click(within(page).getByRole('button', { name: 'Download The Matrix' }));
    const picker = await screen.findByRole('dialog', { name: 'Choose what to download' });
    expect(await within(picker).findByText(/None of your addons has a stream/)).toBeInTheDocument();
    await user.click(within(picker).getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('browses a catalog by genre and page', async () => {
    const user = userEvent.setup();
    commands.stremioCatalog.mockResolvedValue(ok(ROW));
    await AppShellWrapper.mount('/watch/catalog?addon=cinemeta&type=series&catalog=top');
    const page = await screen.findByTestId('watch-catalog-page');
    expect(await within(page).findByRole('heading', { name: 'Popular · Series' })).toBeVisible();
    expect(commands.stremioCatalog).toHaveBeenCalledWith('cinemeta', 'series', 'top', null, 0);

    commands.stremioCatalog.mockResolvedValue(
      ok({ ...ROW, items: [{ ...ROW.items[0], id: 'tt2', name: 'Better Call Saul' }] }),
    );
    await user.click(within(page).getByRole('button', { name: 'More' }));
    expect(await within(page).findByRole('link', { name: 'Better Call Saul' })).toBeVisible();
    expect(commands.stremioCatalog).toHaveBeenLastCalledWith('cinemeta', 'series', 'top', null, 1);

    await user.click(within(page).getByRole('tab', { name: 'Crime' }));
    await vi.waitFor(() =>
      expect(commands.stremioCatalog).toHaveBeenLastCalledWith(
        'cinemeta',
        'series',
        'top',
        'Crime',
        0,
      ),
    );
  });

  it('manages addons: suggestions, by address, and removal', async () => {
    const user = userEvent.setup();
    commands.stremioAddons.mockResolvedValue(ok([ADDON]));
    commands.stremioSuggestions.mockResolvedValue(
      ok([
        { url: ADDON.url, name: 'Cinemeta', description: 'Catalogs', added: true },
        {
          url: 'https://opensubtitles-v3.strem.io/manifest.json',
          name: 'OpenSubtitles v3',
          description: 'Subtitles',
          added: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/browse');
    await user.click(await screen.findByRole('button', { name: 'More options' }));
    await user.click(screen.getByRole('menuitem', { name: 'Addons' }));
    const page = await screen.findByTestId('watch-addons-page');

    const installed = await within(page).findByRole('listitem', { name: 'Cinemeta' });
    expect(within(installed).getByText('Provides: catalogs, details')).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: 'Add Cinemeta' })).not.toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Add OpenSubtitles v3' }));
    expect(commands.stremioAdd).toHaveBeenCalledWith(
      'https://opensubtitles-v3.strem.io/manifest.json',
    );

    commands.stremioAdd.mockResolvedValue({ status: 'error', error: 'The addon answered 404' });
    await user.click(within(page).getByRole('button', { name: 'Add an addon by its address' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add an addon by its address' });
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Add an addon by its address' }),
      'addon.example',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    expect(commands.stremioAdd).toHaveBeenLastCalledWith('addon.example');
    expect(await within(sheet).findByRole('alert')).toHaveTextContent('The addon answered 404');
    await user.click(within(sheet).getByRole('button', { name: 'Close' }));

    await user.click(within(installed).getByRole('button', { name: 'Remove Cinemeta' }));
    expect(commands.stremioRemove).toHaveBeenCalledWith('cinemeta');
  });

  it('searches movies and series from Search', async () => {
    const user = userEvent.setup();
    commands.stremioSearch.mockResolvedValue(ok([{ ...ROW, name: 'Search' }]));
    await AppShellWrapper.mount('/search?type=video');
    await user.type(await screen.findByRole('searchbox', { name: 'Search' }), 'breaking');
    const results = await screen.findByTestId('video-results');
    expect(commands.stremioSearch).toHaveBeenCalledWith('breaking');
    expect(within(results).getByRole('region', { name: 'Series' })).toBeInTheDocument();
    expect(within(results).getByRole('link', { name: 'Breaking Bad' })).toBeVisible();
    expect(within(results).queryByRole('link', { name: /See all/ })).not.toBeInTheDocument();
  });
});
