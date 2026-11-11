import type { MangaCard, OnlineTrack, Settings, StremioItem } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const SAFE = {
  id: 'dev.example.safe/safe',
  name: 'Safe',
  extensionId: 'dev.example.safe',
  enabled: true,
};
const ADULT = {
  id: 'dev.example.adult/adult',
  name: 'Grown-up',
  extensionId: 'dev.example.adult',
  enabled: true,
  adult: true,
};
const TUNES = { id: 'dev.example.music/search', name: 'Tunes', kind: 'music.metadata' };
const LATE_NIGHT = {
  id: 'dev.example.late/search',
  name: 'Late Night',
  kind: 'music.metadata',
  adult: true,
};
const card = (id: string, title: string, sourceId: string, adult = false): MangaCard => ({
  sourceId,
  id,
  title,
  coverUrl: null,
  inLibrary: false,
  ...(adult ? { adult: true } : {}),
});
const track = (title: string, explicit = false): OnlineTrack => ({
  externalId: title,
  title,
  artist: 'Band',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
  ...(explicit ? { explicit: true } : {}),
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

describe('Search filter pill', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok([SAFE, ADULT]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources: string[]) =>
      ok(
        sources[0] === ADULT.id
          ? [
              {
                source: ADULT,
                items: [card('a1', 'Frieren Nights', ADULT.id)],
                hasNextPage: false,
                error: null,
              },
            ]
          : [
              {
                source: SAFE,
                items: [card('s1', 'Frieren', SAFE.id), card('s2', 'Frieren Uncut', SAFE.id, true)],
                hasNextPage: false,
                error: null,
              },
            ],
      ),
    );
  });

  it('is hidden while 18+ content is off, and there is no explicit switch', async () => {
    keepSettings({ showNsfw: false, explicitMusic: false });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByText('Frieren')).toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: 'No 18+' })).not.toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: /explicit/i })).not.toBeInTheDocument();
    expect(screen.queryByText('Frieren Uncut')).not.toBeInTheDocument();
  });

  it('starts on as Rust stores it once 18+ content is on, hiding 18+ results', async () => {
    keepSettings({ showNsfw: true, searchFilter: { noAdult: true } });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByRole('switch', { name: 'No 18+' })).toBeChecked();
    expect(await screen.findByText('Frieren')).toBeInTheDocument();
    expect(screen.queryByText('Frieren Uncut')).not.toBeInTheDocument();
    expect(screen.queryByText('Frieren Nights')).not.toBeInTheDocument();
  });

  it('leaves adult sources unasked and adult titles out when switched on, and is saved', async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByText('Frieren Uncut')).toBeInTheDocument();
    expect(await screen.findByText('Frieren Nights')).toBeInTheDocument();

    const noAdult = screen.getByRole('switch', { name: 'No 18+' });
    expect(noAdult).not.toBeChecked();
    await user.click(noAdult);

    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ noAdult: true, adultOnly: false }),
    });
    await waitFor(() => expect(screen.queryByText('Frieren Uncut')).not.toBeInTheDocument());
    expect(screen.queryByText('Frieren Nights')).not.toBeInTheDocument();
    expect(screen.getByText('Frieren')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'No 18+' })).toBeChecked();
    expect(screen.queryByRole('button', { name: 'Grown-up' })).not.toBeInTheDocument();
  });

  it("hides 18+ music sources' songs and artists in Relevant once switched on", async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true, explicitMusic: true });
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          { provider: TUNES, tracks: [track('Clean Song')], error: null },
          { provider: LATE_NIGHT, tracks: [track('After Hours Song')], error: null },
        ],
      }),
    );
    commands.musicSearchArtists.mockResolvedValue(
      ok([
        { provider: TUNES, id: 'a1', name: 'Song Band', imageUrl: null },
        { provider: LATE_NIGHT, id: 'a2', name: 'Song Club', imageUrl: null },
      ]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=song');
    expect(await screen.findByTestId('relevant-music')).toBeInTheDocument();
    expect(await screen.findByText('After Hours Song')).toBeInTheDocument();
    expect(await screen.findByText('Song Club')).toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: 'No 18+' }));
    await waitFor(() => expect(screen.queryByText('After Hours Song')).not.toBeInTheDocument());
    expect(screen.queryByText('Song Club')).not.toBeInTheDocument();
    expect(screen.getAllByText('Clean Song').length).toBeGreaterThan(0);
    expect(screen.getAllByText('Song Band').length).toBeGreaterThan(0);
  });

  it('leaves explicit songs out while Explicit music is off in Settings', async () => {
    keepSettings({ showNsfw: true, explicitMusic: false });
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          { provider: TUNES, tracks: [track('Clean Song'), track('Rude Song', true)], error: null },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=song');
    expect((await screen.findAllByText('Clean Song')).length).toBeGreaterThan(0);
    expect(screen.queryByText('Rude Song')).not.toBeInTheDocument();
  });
});

/** A song with a stream of its own. */
const streaming = (title: string, explicit = false): OnlineTrack => ({
  ...track(title, explicit),
  source: { kind: 'url', url: `https://example.com/${title}.mp3`, headers: {} },
});

const movie = (id: string, name: string, playable: boolean): StremioItem => ({
  kind: 'movie',
  id,
  name,
  poster: null,
  posterShape: 'poster',
  releaseInfo: null,
  rating: null,
  playable,
});

const openFilters = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(await screen.findByRole('button', { name: /^Search filters/ }));
  return screen.findByRole('dialog', { name: 'Search filters' });
};

describe('Search filters', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(ok([SAFE, ADULT]));
    commands.catalogGlobalSearch.mockImplementation(async (_medium, _query, sources: string[]) =>
      ok(
        sources[0] === ADULT.id
          ? [
              {
                source: ADULT,
                items: [card('a1', 'Frieren Nights', ADULT.id)],
                hasNextPage: false,
                error: null,
              },
            ]
          : [
              {
                source: SAFE,
                items: [
                  card('s1', 'Frieren', SAFE.id),
                  card('s2', 'Frieren Uncut', SAFE.id, true),
                  { ...card('s3', 'Frieren Saved', SAFE.id), inLibrary: true },
                ],
                hasNextPage: false,
                error: null,
              },
            ],
      ),
    );
  });

  it('shows only 18+ titles under 18+ only, which turns No 18+ off, and counts on the button', async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true, searchFilter: { noAdult: true } });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByText('Frieren')).toBeInTheDocument();
    expect(screen.queryByText('Frieren Nights')).not.toBeInTheDocument();

    const sheet = await openFilters(user);
    const adultOnly = within(sheet).getByRole('switch', { name: '18+ only' });
    expect(adultOnly).not.toBeChecked();
    await user.click(adultOnly);

    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ noAdult: false, adultOnly: true }),
    });
    expect(within(sheet).getByRole('switch', { name: '18+ only' })).toBeChecked();
    expect(screen.getByRole('switch', { name: 'No 18+' })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Search filters, 1 on' })).toBeInTheDocument();
    // Titles marked 18+, and everything the 18+ source has (it's asked now).
    expect(await screen.findByText('Frieren Nights')).toBeInTheDocument();
    expect(screen.getByText('Frieren Uncut')).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText('Frieren')).not.toBeInTheDocument());

    // Switching the pill on again turns 18+ only off.
    await user.click(screen.getByRole('switch', { name: 'No 18+' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ noAdult: true, adultOnly: false }),
    });
    expect(within(sheet).getByRole('switch', { name: '18+ only' })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Search filters' })).toBeInTheDocument();
    expect(await screen.findByText('Frieren')).toBeInTheDocument();
    expect(screen.queryByText('Frieren Nights')).not.toBeInTheDocument();
  });

  it('leaves titles in the library out under Not in library, for comics alone', async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: false });
    await AppShellWrapper.mount('/browse?tab=search&type=manga&q=frieren');
    expect(await screen.findByText('Frieren Saved')).toBeInTheDocument();

    const sheet = await openFilters(user);
    // 18+ content is off: there's nothing 18+ to ask for.
    expect(within(sheet).queryByRole('switch', { name: '18+ only' })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole('switch', { name: 'Not in library' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ hideSavedComics: true, hideSavedBooks: false }),
    });
    await waitFor(() => expect(screen.queryByText('Frieren Saved')).not.toBeInTheDocument());
    expect(screen.getByText('Frieren')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Search filters, 1 on' })).toBeInTheDocument();
  });

  it('keeps the titles an addon gives streams for under Playable only, and says how', async () => {
    const user = userEvent.setup();
    keepSettings({});
    commands.videoSources.mockResolvedValue(ok([]));
    commands.stremioCatalogs.mockResolvedValue(
      ok([
        {
          addonId: 'cinemeta',
          addonName: 'Cinemeta',
          kind: 'movie',
          catalogId: 'top',
          name: 'Popular',
          extras: [],
          board: true,
          searchable: true,
          discoverable: true,
          pages: true,
          failing: false,
        },
      ]),
    );
    commands.stremioSearchCatalog.mockResolvedValue(
      ok({
        addonId: 'cinemeta',
        addonName: 'Cinemeta',
        kind: 'movie',
        catalogId: 'top',
        name: 'Popular',
        items: [movie('tt1', 'Sintel', true), movie('yt1', 'Sintel Trailer', false)],
        genres: [],
        hasMore: false,
        error: null,
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=video&q=sintel');
    const results = await screen.findByTestId('video-results');
    expect(within(results).getByText('Sintel Trailer')).toBeInTheDocument();

    const sheet = await openFilters(user);
    expect(within(sheet).getByText(/an addon you have gives streams/)).toBeInTheDocument();
    await user.click(within(sheet).getByRole('switch', { name: 'Playable only' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ playableVideos: true }),
    });
    await waitFor(() =>
      expect(screen.getByTestId('video-results')).not.toHaveTextContent('Sintel Trailer'),
    );
    expect(within(screen.getByTestId('video-results')).getByText('Sintel')).toBeInTheDocument();
  });

  it("narrows Audio's songs to explicit ones, or to ones that stream", async () => {
    const user = userEvent.setup();
    keepSettings({ explicitMusic: true });
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: TUNES,
            tracks: [
              streaming('Clean Song'),
              streaming('Rude Song', true),
              track('Described Song'),
            ],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=song');
    expect((await screen.findAllByText('Described Song')).length).toBeGreaterThan(0);
    expect(commands.onlineSearch).toHaveBeenCalledWith('song', true);

    const sheet = await openFilters(user);
    await user.click(within(sheet).getByRole('switch', { name: 'Explicit only' }));
    await waitFor(() => expect(screen.queryByText('Clean Song')).not.toBeInTheDocument());
    expect(screen.queryByText('Described Song')).not.toBeInTheDocument();
    expect(screen.getAllByText('Rude Song').length).toBeGreaterThan(0);

    await user.click(within(sheet).getByRole('switch', { name: 'Explicit only' }));
    await user.click(within(sheet).getByRole('switch', { name: 'Playable only' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'searchFilter',
      value: expect.objectContaining({ explicitOnly: false, playableSongs: true }),
    });
    expect((await screen.findAllByText('Clean Song')).length).toBeGreaterThan(0);
    expect(screen.getAllByText('Rude Song').length).toBeGreaterThan(0);
    expect(screen.queryByText('Described Song')).not.toBeInTheDocument();
  });

  it('offers each kind its own filters, and no button where nothing could be filtered', async () => {
    const user = userEvent.setup();
    keepSettings({ explicitMusic: false });
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=song');
    const sheet = await openFilters(user);
    // Explicit songs are off in Settings: nothing explicit to keep.
    expect(within(sheet).queryByRole('switch', { name: 'Explicit only' })).not.toBeInTheDocument();
    expect(within(sheet).getByRole('switch', { name: 'Playable only' })).toBeInTheDocument();
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

    // Artists carry nothing the song filters look at.
    await user.click(screen.getByRole('button', { name: 'Artists' }));
    await waitFor(() =>
      expect(screen.queryByRole('button', { name: /^Search filters/ })).not.toBeInTheDocument(),
    );
  });

  it("lists every kind's filters in All", async () => {
    const user = userEvent.setup();
    keepSettings({ showNsfw: true, searchFilter: { noAdult: false, hideSavedBooks: true } });
    await AppShellWrapper.mount('/browse?tab=search&q=frieren');
    expect(await screen.findByRole('button', { name: 'Search filters, 1 on' })).toBeInTheDocument();
    const sheet = await openFilters(user);
    expect(within(sheet).getByRole('switch', { name: '18+ only' })).toBeInTheDocument();
    for (const group of ['Songs', 'Videos', 'Comics', 'Books']) {
      expect(within(sheet).getByRole('region', { name: group })).toBeInTheDocument();
    }
    expect(
      within(within(sheet).getByRole('region', { name: 'Books' })).getByRole('switch', {
        name: 'Not in library',
      }),
    ).toBeChecked();
  });
});
