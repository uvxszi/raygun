import type { MangaCard, OnlineTrack, Settings } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';
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
      value: { noAdult: true },
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
