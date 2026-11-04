import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const PROVIDER = {
  id: 'dev.example.tunes/catalogue',
  extensionId: 'dev.example.tunes',
  kind: 'music.metadata',
  name: 'Tunes',
};
const PLAYLIST = {
  provider: PROVIDER,
  id: '42',
  title: 'Workout',
  author: 'Rudy - Tunes Moods Editor',
  artworkUrl: null,
  trackCount: 38,
  featured: true,
};

describe('Playlist search (M46.39)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.musicSearchPlaylists.mockResolvedValue(ok([PLAYLIST]));
    commands.musicPlaylist.mockResolvedValue(
      ok({
        playlist: PLAYLIST,
        tracks: [
          {
            externalId: '1',
            title: 'Stronger',
            artist: 'Kanye West',
            album: null,
            durationMs: 311_000,
            artworkUrl: null,
            source: null,
            explicit: true,
          },
        ],
      }),
    );
    commands.playlistCreate.mockResolvedValue(
      ok({ id: 'p1', name: 'Workout', itemCount: 0, artworkUrls: [], updatedAt: 0 } as never),
    );
    commands.playlistAdd.mockResolvedValue(ok(null));
  });

  it('finds playlists, featured ones apart, and saves one to the library', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=playlists&q=workout');
    await screen.findByTestId('playlist-finds');
    expect(commands.musicSearchPlaylists).toHaveBeenCalledWith('workout', false);
    await user.click(screen.getByRole('button', { name: 'Featured' }));
    expect(commands.musicSearchPlaylists).toHaveBeenLastCalledWith('workout', true);

    // The featured list replaces the first one.
    const finds = await screen.findByTestId('playlist-finds');
    await user.click(within(finds).getByRole('link', { name: /Workout/ }));
    const page = await screen.findByTestId('music-playlist-page');
    expect(await within(page).findByText('Stronger')).toBeInTheDocument();
    // Explicit songs carry the E (M46.36).
    expect(within(page).getByRole('img', { name: 'Explicit' })).toBeInTheDocument();
    await user.click(within(page).getByRole('button', { name: 'Save to library' }));
    await vi.waitFor(() =>
      expect(commands.playlistAdd).toHaveBeenCalledWith(
        'p1',
        [expect.objectContaining({ title: 'Stronger' })],
        false,
      ),
    );
    expect(commands.playlistCreate).toHaveBeenCalledWith('Workout');
  });
});
