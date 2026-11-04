import type { OnlineTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const KEPT = [
  { query: 'daft punk', kind: 'music', searchedAt: 2 },
  { query: 'frieren', kind: 'manga', searchedAt: 1 },
];

const TUNES = {
  id: 'dev.example.tunes/catalogue',
  extensionId: 'dev.example.tunes',
  kind: 'music.metadata',
  name: 'Tunes',
};

const song = (title: string): OnlineTrack => ({
  externalId: title,
  title,
  artist: 'Daft Punk',
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
});

describe('Recent searches', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.searchRecent.mockResolvedValue(ok(KEPT));
  });

  it('shows the searches made lately before anything is typed, each searched again with a tap', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/browse?tab=search');
    const recent = await screen.findByRole('region', { name: 'Recent searches' });
    expect(within(recent).getByRole('button', { name: /^daft punk/ })).toHaveTextContent('Audio');

    await user.click(within(recent).getByRole('button', { name: /^frieren/ }));
    expect(await screen.findByRole('tab', { name: 'Comics' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
    expect(screen.getByRole('searchbox', { name: 'Search' })).toHaveValue('frieren');
    expect(commands.searchRecentAdd).toHaveBeenCalledWith('frieren', 'manga');
    expect(screen.queryByRole('region', { name: 'Recent searches' })).not.toBeInTheDocument();
  });

  it('forgets one search, or every one', async () => {
    const user = userEvent.setup();
    commands.searchRecentRemove.mockResolvedValue(ok([KEPT[1]]));
    await AppShellWrapper.mount('/browse?tab=search&type=music');
    const recent = await screen.findByRole('region', { name: 'Recent searches' });
    await user.click(within(recent).getByRole('button', { name: 'Forget “daft punk”' }));
    expect(commands.searchRecentRemove).toHaveBeenCalledWith('daft punk', 'music');
    await vi.waitFor(() =>
      expect(within(recent).queryByRole('button', { name: /^daft punk/ })).not.toBeInTheDocument(),
    );
    await user.click(within(recent).getByRole('button', { name: 'Clear' }));
    expect(commands.searchRecentClear).toHaveBeenCalled();
    await vi.waitFor(() =>
      expect(screen.queryByRole('region', { name: 'Recent searches' })).not.toBeInTheDocument(),
    );
  });

  it('remembers a search when Enter is pressed, or when something it found is played', async () => {
    const user = userEvent.setup();
    commands.searchRecent.mockResolvedValue(ok([]));
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: TUNES, tracks: [song('Get Lucky')], error: null }],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs');
    await user.type(screen.getByRole('searchbox', { name: 'Search' }), 'get lucky{Enter}');
    await vi.waitFor(() =>
      expect(commands.searchRecentAdd).toHaveBeenLastCalledWith('get lucky', 'music'),
    );

    commands.searchRecentAdd.mockClear();
    await user.clear(screen.getByRole('searchbox', { name: 'Search' }));
    await user.type(screen.getByRole('searchbox', { name: 'Search' }), 'lucky');
    await vi.waitFor(() => expect(commands.onlineSearch).toHaveBeenLastCalledWith('lucky'));
    await screen.findAllByTestId('online-track-row');
    // Switching pills isn't a search made.
    await user.click(screen.getByRole('button', { name: 'Songs' }));
    expect(commands.searchRecentAdd).not.toHaveBeenCalled();
    const [row] = await screen.findAllByTestId('online-track-row');
    await user.click(row!);
    await vi.waitFor(() => expect(commands.searchRecentAdd).toHaveBeenCalledWith('lucky', 'music'));
  });

  it('keeps and shows nothing while search history is off', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, privacy: { ...DEFAULT_SETTINGS.privacy, searchHistory: false } }),
    );
    await AppShellWrapper.mount('/browse?tab=search');
    await screen.findByTestId('search-page');
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByRole('region', { name: 'Recent searches' })).not.toBeInTheDocument();
    expect(commands.searchRecent).not.toHaveBeenCalled();
  });
});
