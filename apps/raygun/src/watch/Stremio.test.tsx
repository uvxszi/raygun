import type {
  StremioAddon,
  StremioCatalogInfo,
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

const catalog = (extra: Partial<StremioCatalogInfo> = {}): StremioCatalogInfo => ({
  addonId: 'cinemeta',
  addonName: 'Cinemeta',
  kind: 'series',
  catalogId: 'top',
  name: 'Popular',
  extras: [{ name: 'genre', required: false, options: ['Drama', 'Crime'] }],
  board: true,
  searchable: true,
  discoverable: true,
  pages: true,
  failing: false,
  ...extra,
});

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
  continueVideoId: null,
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
  ratings: [],
  cast: ['Bryan Cranston'],
  director: [],
  trailerYtIds: [],
  videos: [episode(1, 1, { watched: true }), episode(1, 2), episode(2, 1)],
  positionMs: 0,
  durationMs: null,
  watched: false,
  downloaded: false,
  addonName: 'Cinemeta',
  addonId: 'cinemeta',
  videoId: 'tt0903747',
  live: false,
  inLibrary: false,
  titleId: null,
};

const MOVIE: StremioMeta = {
  ...SERIES,
  kind: 'movie',
  id: 'tt0133093',
  videoId: 'tt0133093',
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
  filename: null,
  size: null,
  videoHash: null,
  notWebReady: false,
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
  enabled: true,
  provides: [
    { resource: 'catalog', kind: 'movie', count: 2 },
    { resource: 'catalog', kind: 'series', count: 1 },
    { resource: 'meta', kind: 'series', count: 1 },
  ],
  p2p: false,
  configureUrl: null,
  needsSetup: false,
  failing: false,
};

const SUBTITLES: StremioAddon = {
  ...ADDON,
  id: 'subs',
  url: 'https://opensubtitles-v3.strem.io/manifest.json',
  name: 'OpenSubtitles v3',
  description: null,
  resources: ['subtitles'],
  catalogs: 0,
  provides: [{ resource: 'subtitles', kind: null, count: 1 }],
  failing: true,
};

describe('Stremio addons in Watch', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows catalog rows, opens a series and plays an episode stream', async () => {
    const user = userEvent.setup();
    commands.stremioCatalogs.mockResolvedValue(ok([catalog()]));
    commands.stremioCatalog.mockResolvedValue(ok(ROW));
    commands.stremioMeta.mockResolvedValue(ok(SERIES));
    commands.stremioStreams.mockResolvedValue(
      ok([
        {
          addonId: 'local',
          addonName: 'Local streams',
          streams: [
            stream('torrent', 'Torrent 1080p', 'magnet:?xt=urn:btih:abc'),
            stream('direct', 'HLS 720p', 'https://cdn.example/e2.m3u8'),
          ],
          error: null,
          failing: false,
        },
      ]),
    );
    commands.stremioAddons.mockResolvedValue(ok([ADDON]));
    await AppShellWrapper.mount('/');
    // Addons with catalogues are sources in Browse; each opens its catalogue rows.
    await user.click(screen.getByTestId('nav-browse-tab'));
    // Browse opens on Browse all; the addons are under Sources.
    await user.click(await screen.findByTestId('browse-tab-sources'));
    await user.click(await screen.findByRole('link', { name: 'Cinemeta' }));
    await screen.findByTestId('stremio-addon-page');

    const row = await screen.findByRole('region', { name: 'Popular · Series' });
    // The row asks for its first page with nothing chosen.
    expect(commands.stremioCatalog).toHaveBeenCalledWith('cinemeta', 'series', 'top', [], 0);
    await user.click(await within(row).findByRole('link', { name: 'Breaking Bad' }));
    const page = await screen.findByTestId('watch-title-page');
    expect(commands.stremioMeta).toHaveBeenCalledWith('series', 'tt0903747');
    expect(within(page).getByText('2008–2013 · 49 min · ★ 9.5')).toBeInTheDocument();

    // Season 1 opens first: it has an unwatched episode.
    expect(within(page).getByRole('tab', { name: 'Season 1' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(within(page).getByRole('button', { name: 'Mark S1 E1 unwatched' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );

    // A name that only repeats the number isn't repeated: "S1 E2", not "S1 E2 · Episode 2".
    const second = within(page).getByRole('listitem', { name: 'S1 E2' });
    // A tap watches: the player opens and finds the best stream itself (M44).
    await user.click(within(second).getByRole('button', { name: /^Episode 2/ }));
    expect(commands.stremioWatch).toHaveBeenCalledWith('series', 'tt0903747', 'tt0903747:1:2');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();

    // A long press picks the stream.
    await user.pointer({
      keys: '[MouseRight]',
      target: within(second).getByRole('button', { name: /^Episode 2/ }),
    });
    const picker = await screen.findByRole('dialog', { name: 'Streams' });
    // Streams are asked for with the series, whose details say which ids its episodes go by.
    expect(commands.stremioStreams).toHaveBeenCalledWith('series', 'tt0903747', 'tt0903747:1:2');
    // While torrents are off, they're folded away with how many there are, and what they need.
    expect(
      within(picker).queryByRole('button', { name: /^Torrent 1080p/ }),
    ).not.toBeInTheDocument();
    await user.click(
      await within(picker).findByRole('button', { name: '1 torrent (torrents are off)' }),
    );
    expect(within(picker).getByText('Torrent 1080p')).toBeInTheDocument();
    expect(
      within(picker).getByText('Torrents are off: Settings → Video player → Torrents'),
    ).toBeInTheDocument();
    await user.click(within(picker).getByRole('button', { name: 'More for Torrent 1080p' }));
    expect(screen.getByRole('menuitem', { name: 'Copy magnet link' })).toBeInTheDocument();

    await user.click(within(picker).getByRole('button', { name: /^HLS 720p/ }));
    expect(commands.stremioPlay).toHaveBeenCalledWith(
      {
        kind: 'series',
        metaId: 'tt0903747',
        videoId: 'tt0903747:1:2',
        title: 'Breaking Bad',
        subtitle: 'S1 E2',
        poster: null,
        stream: stream('direct', 'HLS 720p', 'https://cdn.example/e2.m3u8'),
        external: false,
      },
      'en',
    );
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    await user.click(within(page).getByRole('tab', { name: 'Season 2' }));
    await user.click(within(page).getByRole('button', { name: 'Mark S2 E1 watched' }));
    expect(commands.stremioSetWatched).toHaveBeenCalledWith(
      'series',
      'tt0903747:2:1',
      'Breaking Bad',
      'S2 E1',
      null,
      true,
    );
  });

  it('picks among many seasons from a list instead of pills', async () => {
    const user = userEvent.setup();
    const seasons = Array.from({ length: 9 }, (_, index) => index + 1);
    commands.stremioMeta.mockResolvedValue(
      ok({ ...SERIES, videos: seasons.map((season) => episode(season, 1)) }),
    );
    await AppShellWrapper.mount('/watch/title?type=series&id=tt0903747');
    const page = await screen.findByTestId('watch-title-page');
    expect(within(page).queryByRole('tablist', { name: 'Seasons' })).not.toBeInTheDocument();
    await user.click(within(page).getByRole('button', { name: 'Season 1, choose a season' }));
    const list = await screen.findByRole('listbox', { name: 'Seasons' });
    expect(within(list).getAllByRole('option')).toHaveLength(9);
    await user.click(within(list).getByRole('option', { name: 'Season 7' }));
    expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
    expect(within(page).getByRole('listitem', { name: 'S7 E1' })).toBeInTheDocument();
    expect(within(page).queryByRole('listitem', { name: 'S1 E1' })).not.toBeInTheDocument();
    expect(within(page).getByRole('button', { name: 'Season 7, choose a season' })).toBeVisible();
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

  it('finds the title at the video sources, beside the addons', async () => {
    const user = userEvent.setup();
    commands.stremioMeta.mockResolvedValue(ok(MOVIE));
    const films = {
      id: 'dev.example/films',
      name: 'Public Films',
      extensionId: 'dev.example',
      enabled: true,
    };
    commands.videoSources.mockResolvedValue(ok([films]));
    commands.catalogGlobalSearch.mockResolvedValue(
      ok([
        {
          source: films,
          items: [
            {
              sourceId: films.id,
              id: 'matrix',
              title: 'The Matrix',
              coverUrl: null,
              inLibrary: false,
            },
          ],
          hasNextPage: false,
          error: null,
        },
      ]),
    );
    commands.videoTitle.mockResolvedValue({ status: 'error' as const, error: 'gone' });
    // Another addon lists it by another id; this page's own id and other films stay out.
    commands.stremioCatalogs.mockResolvedValue(
      ok([
        {
          addonId: 'anime',
          addonName: 'Anime Catalogs',
          kind: 'movie',
          catalogId: 'search',
          name: 'Search',
          extras: [],
          board: false,
          searchable: true,
          discoverable: false,
          pages: false,
          failing: false,
        },
      ]),
    );
    commands.stremioSearchCatalog.mockResolvedValue(
      ok({
        addonId: 'anime',
        addonName: 'Anime Catalogs',
        kind: 'movie',
        catalogId: 'search',
        name: 'Search',
        items: [
          {
            kind: 'movie',
            id: 'tt0133093',
            name: 'The Matrix',
            poster: null,
            posterShape: 'poster',
            releaseInfo: '1999',
            rating: null,
          },
          {
            kind: 'movie',
            id: 'kitsu:603',
            name: 'The Matrix',
            poster: null,
            posterShape: 'poster',
            releaseInfo: '1999',
            rating: null,
          },
          {
            kind: 'movie',
            id: 'tt9',
            name: 'Gardening for Beginners',
            poster: null,
            posterShape: 'poster',
            releaseInfo: null,
            rating: null,
          },
        ],
        genres: [],
        hasMore: false,
        error: null,
      }),
    );
    await AppShellWrapper.mount('/watch/title?type=movie&id=tt0133093');
    const page = await screen.findByTestId('watch-title-page');
    await user.click(within(page).getByRole('button', { name: 'Find in other sources' }));
    const section = await screen.findByTestId('other-sources');
    expect(await within(section).findByText('Public Films')).toBeInTheDocument();
    expect(commands.catalogGlobalSearch).toHaveBeenCalledWith('video', 'The Matrix', [films.id]);
    // A video has no author to ask for.
    expect(within(section).queryByRole('textbox', { name: 'Author' })).not.toBeInTheDocument();
    // The Stremio addons' copy opens its own page.
    const addons = await within(section).findByRole('region', { name: 'In Stremio addons' });
    const links = within(addons).getAllByRole('link', { name: 'The Matrix' });
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute('href', expect.stringContaining('id=kitsu%3A603'));
    expect(within(addons).getByText('Anime Catalogs')).toBeInTheDocument();
    expect(within(addons).queryByText('Gardening for Beginners')).not.toBeInTheDocument();
  });

  it('plays a movie a catalog lists by another id by the id its details give', async () => {
    const user = userEvent.setup();
    // A TMDB addon's movie: its page goes by TMDB's id, its streams by IMDb's.
    commands.stremioMeta.mockResolvedValue(
      ok({ ...MOVIE, id: 'tmdb:603', videoId: 'tt0133093', addonName: 'TMDB' }),
    );
    commands.stremioStreams.mockResolvedValue(
      ok([
        {
          addonId: 'links',
          addonName: 'Links',
          streams: [stream('title', 'The Matrix Reloaded', 'movie/tt0234215')],
          error: null,
          failing: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/watch/title?type=movie&id=tmdb%3A603');
    const page = await screen.findByTestId('watch-title-page');
    expect(commands.stremioMeta).toHaveBeenCalledWith('movie', 'tmdb:603');

    await user.click(within(page).getByRole('button', { name: 'Play' }));
    expect(commands.stremioWatch).toHaveBeenCalledWith('movie', 'tmdb:603', null);
    await user.click(within(page).getByRole('button', { name: 'Mark watched' }));
    expect(commands.stremioSetWatched).toHaveBeenLastCalledWith(
      'movie',
      'tt0133093',
      'The Matrix',
      '1999',
      null,
      true,
    );

    // A stream that is another title opens its page.
    await user.click(within(page).getByRole('button', { name: 'Choose a source' }));
    const picker = await screen.findByRole('dialog', { name: 'Streams' });
    expect(commands.stremioStreams).toHaveBeenCalledWith('movie', 'tmdb:603', 'tt0133093');
    await user.click(await within(picker).findByRole('button', { name: /^The Matrix Reloaded/ }));
    await vi.waitFor(() =>
      expect(commands.stremioMeta).toHaveBeenLastCalledWith('movie', 'tt0234215'),
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('browses Discover by type, catalog, choice and page', async () => {
    const user = userEvent.setup();
    commands.stremioCatalogs.mockResolvedValue(
      ok([
        catalog(),
        catalog({ kind: 'movie', catalogId: 'top', name: 'Popular' }),
        catalog({
          addonId: 'tmdb',
          addonName: 'TMDB',
          kind: 'movie',
          catalogId: 'tmdb.year',
          name: 'Year',
          board: false,
          extras: [{ name: 'genre', required: true, options: ['2026', '2025'] }],
        }),
        // Search-only catalogs aren't browsed.
        catalog({ catalogId: 'search', name: 'Search', board: false, discoverable: false }),
      ]),
    );
    commands.stremioCatalog.mockResolvedValue(ok(ROW));
    await AppShellWrapper.mount('/watch/catalog?addon=cinemeta&type=series&catalog=top');
    const page = await screen.findByTestId('watch-catalog-page');
    expect(await within(page).findByRole('link', { name: 'Breaking Bad' })).toBeVisible();
    expect(commands.stremioCatalog).toHaveBeenCalledWith('cinemeta', 'series', 'top', [], 0);
    // Movies first, as Stremio orders types.
    const types = within(page).getByRole('group', { name: 'Types' });
    expect(
      within(types)
        .getAllByRole('button')
        .map((button) => button.textContent),
    ).toEqual(['Movies', 'Series']);

    commands.stremioCatalog.mockResolvedValue(
      ok({ ...ROW, items: [{ ...ROW.items[0]!, id: 'tt2', name: 'Better Call Saul' }] }),
    );
    await user.click(within(page).getByRole('button', { name: 'More' }));
    expect(await within(page).findByRole('link', { name: 'Better Call Saul' })).toBeVisible();
    expect(commands.stremioCatalog).toHaveBeenLastCalledWith('cinemeta', 'series', 'top', [], 1);

    const genres = within(page).getByRole('group', { name: 'Genres' });
    await user.click(within(genres).getByRole('button', { name: 'Crime' }));
    await vi.waitFor(() =>
      expect(commands.stremioCatalog).toHaveBeenLastCalledWith(
        'cinemeta',
        'series',
        'top',
        [{ name: 'genre', value: 'Crime' }],
        0,
      ),
    );

    // A catalog that needs a choice starts on its first one, with no "All".
    await user.click(within(types).getByRole('button', { name: 'Movies' }));
    const catalogs = await within(page).findByRole('group', { name: 'Catalogs' });
    await user.click(within(catalogs).getByRole('button', { name: /^Year/ }));
    await vi.waitFor(() =>
      expect(commands.stremioCatalog).toHaveBeenLastCalledWith(
        'tmdb',
        'movie',
        'tmdb.year',
        [{ name: 'genre', value: '2026' }],
        0,
      ),
    );
    const years = within(page).getByRole('group', { name: 'Genres' });
    expect(within(years).queryByRole('button', { name: 'All' })).not.toBeInTheDocument();
    expect(within(years).getByRole('button', { name: '2026' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('manages addons: what they provide, adding after a look, order, off and removal', async () => {
    const user = userEvent.setup();
    commands.stremioAddons.mockResolvedValue(ok([ADDON, SUBTITLES]));
    commands.stremioSuggestions.mockResolvedValue(
      ok([
        { url: ADDON.url, name: 'Cinemeta', description: 'Catalogs', added: true },
        {
          url: 'https://watchhub.strem.io/manifest.json',
          name: 'WatchHub',
          description: 'Streams',
          added: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/browse?tab=sources');
    await user.click(await screen.findByRole('button', { name: 'More options' }));
    await user.click(screen.getByRole('menuitem', { name: 'Sources' }));
    const page = await screen.findByTestId('sources-page');
    const kinds = within(page).getByRole('navigation', { name: 'Kinds of sources' });
    await user.click(await within(kinds).findByRole('link', { name: 'Addons · 2' }));

    const installed = await within(page).findByRole('listitem', { name: 'Cinemeta' });
    const provides = within(installed).getByRole('list', { name: 'What Cinemeta provides' });
    expect(
      within(provides)
        .getAllByRole('listitem')
        .map((chip) => chip.textContent),
    ).toEqual(['Movie catalogs · 2', 'Series catalog', 'Series details']);
    const subtitles = within(page).getByRole('listitem', { name: 'OpenSubtitles v3' });
    expect(within(subtitles).getByText("Didn't answer lately")).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: 'Add Cinemeta' })).not.toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Add WatchHub' }));
    expect(commands.stremioAdd).toHaveBeenCalledWith('https://watchhub.strem.io/manifest.json');

    // By address, in Sources' one Add: a look at what it is first, then Add.
    commands.sourcesDetect.mockResolvedValue({
      status: 'error',
      error: 'Raygun found no repository, addon, server or extension at addon.example',
    });
    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
    const field = within(sheet).getByRole('textbox', { name: 'Address' });
    await user.type(field, 'addon.example');
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    expect(commands.sourcesDetect).toHaveBeenLastCalledWith('addon.example');
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Raygun found no repository, addon, server or extension at addon.example',
    );

    const tmdb: StremioAddon = {
      ...ADDON,
      id: '',
      url: 'https://tmdb.example/manifest.json',
      name: 'TMDB',
      provides: [{ resource: 'catalog', kind: 'movie', count: 12 }],
      configureUrl: 'https://tmdb.example/configure',
    };
    const found = {
      kind: 'addon' as const,
      format: null,
      server: null,
      url: tmdb.url,
      name: 'TMDB',
      description: null,
      added: false,
      addon: tmdb,
      entries: [],
      note: null,
    };
    commands.sourcesDetect.mockResolvedValue(ok(found));
    commands.sourcesAdd.mockResolvedValue(ok({ ...found, added: true }));
    await user.clear(field);
    await user.type(field, 'tmdb.example');
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    const card = await within(sheet).findByRole('region', {
      name: 'Looks like a Stremio addon: TMDB',
    });
    expect(within(card).getByText('Movie catalogs · 12')).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Set up' }));
    expect(commands.stremioConfigure).toHaveBeenCalledWith('https://tmdb.example/manifest.json');
    await user.click(within(card).getByRole('button', { name: 'Add' }));
    expect(commands.sourcesAdd).toHaveBeenLastCalledWith('tmdb.example');
    await vi.waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Add a source' })).toBeNull(),
    );

    // An addon that must be set up first can't be added as it is.
    commands.sourcesDetect.mockResolvedValue(
      ok({ ...found, addon: { ...tmdb, needsSetup: true } }),
    );
    await user.click(within(page).getByRole('button', { name: 'Add a source' }));
    const again = await screen.findByRole('dialog', { name: 'Add a source' });
    await user.type(within(again).getByRole('textbox', { name: 'Address' }), 'tmdb.example');
    await user.click(within(again).getByRole('button', { name: 'Look up' }));
    const setup = await within(again).findByRole('region', {
      name: 'Looks like a Stremio addon: TMDB',
    });
    expect(within(setup).getByText('Needs setting up on its website first')).toBeInTheDocument();
    expect(within(setup).queryByRole('button', { name: 'Add' })).not.toBeInTheDocument();
    await user.click(within(again).getByRole('button', { name: 'Close' }));

    // Order is the order details are asked in.
    await user.click(within(installed).getByRole('button', { name: 'More for Cinemeta' }));
    expect(screen.queryByRole('menuitem', { name: 'Move up' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Move down' }));
    expect(commands.stremioSetOrder).toHaveBeenCalledWith(['subs', 'cinemeta']);

    await user.click(within(installed).getByRole('button', { name: 'More for Cinemeta' }));
    await user.click(screen.getByRole('menuitem', { name: 'Turn off' }));
    expect(commands.stremioSetEnabled).toHaveBeenCalledWith('cinemeta', false);

    await user.click(within(installed).getByRole('button', { name: 'More for Cinemeta' }));
    await user.click(screen.getByRole('menuitem', { name: 'Remove Cinemeta' }));
    expect(commands.stremioRemove).toHaveBeenCalledWith('cinemeta');
  });

  it('searches movies and series from Search, each addon as it answers', async () => {
    const user = userEvent.setup();
    commands.stremioCatalogs.mockResolvedValue(
      ok([catalog({ catalogId: 'search', name: 'Search', board: false })]),
    );
    commands.stremioSearchCatalog.mockResolvedValue(ok({ ...ROW, name: 'Search' }));
    await AppShellWrapper.mount('/search?type=video');
    await user.type(await screen.findByRole('searchbox', { name: 'Search' }), 'breaking');
    const results = await screen.findByTestId('video-results');
    await vi.waitFor(() =>
      expect(commands.stremioSearchCatalog).toHaveBeenCalledWith(
        'cinemeta',
        'series',
        'search',
        'breaking',
      ),
    );
    expect(await within(results).findByRole('region', { name: 'Series' })).toBeInTheDocument();
    expect(within(results).getByRole('link', { name: 'Breaking Bad' })).toBeVisible();
    expect(within(results).queryByRole('link', { name: /See all/ })).not.toBeInTheDocument();
  });
});
