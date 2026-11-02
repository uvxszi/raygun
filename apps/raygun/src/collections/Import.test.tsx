import type { ImportedList, ImportedSong, MatchedSong } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const song = (title: string, artist: string): ImportedSong => ({
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
});

const list = (songs: ImportedSong[], capped = false): ImportedList => ({
  name: "Today's Top Hits",
  artworkUrl: null,
  songs,
  capped,
});

const SONGS = [song('Loser', 'Tame Impala'), song('Housewife', 'Tate McRae')];

describe('Importing songs', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.importMatch.mockImplementation(async (songs: ImportedSong[]) =>
      ok(
        songs.map((one): MatchedSong => ({
          song: { ...one, artworkUrl: 'https://img.example/a.jpg' },
          matchedBy: one.title === 'Loser' ? 'Deezer' : null,
          source: null,
        })),
      ),
    );
  });

  it('reads a Spotify link, matches the songs and makes a playlist', async () => {
    const user = userEvent.setup();
    commands.importReadLink.mockResolvedValue(ok(list(SONGS, true)));
    commands.importSave.mockResolvedValue(ok('p-new'));
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Playlist, album or song link' }),
      'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    expect(commands.importReadLink).toHaveBeenCalledWith(
      'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M',
    );

    const review = await within(sheet).findByTestId('import-review');
    expect(await within(review).findByText(/Found 1 of 2 in your sources/)).toBeInTheDocument();
    expect(review).toHaveTextContent("Spotify shares only a playlist's first 100 songs");
    expect(within(review).getByLabelText('Found on Deezer')).toBeInTheDocument();
    expect(within(review).getByLabelText('Not found yet')).toBeInTheDocument();

    // Untick one, rename, and save.
    await user.click(within(review).getByRole('checkbox', { name: /Housewife/ }));
    const name = within(review).getByRole('textbox', { name: 'Playlist name' });
    await user.clear(name);
    await user.type(name, 'Hits');
    await user.click(within(review).getByRole('button', { name: 'Make a playlist of 1 song' }));
    expect(commands.importSave).toHaveBeenCalledWith('Hits', [
      expect.objectContaining({
        song: expect.objectContaining({ title: 'Loser' }),
        matchedBy: 'Deezer',
      }),
    ]);
  });

  it('reads pasted lines and says calmly when a link fails', async () => {
    const user = userEvent.setup();
    commands.importReadLink.mockResolvedValue({
      status: 'error',
      error: 'Spotify has no such list; it may be private or removed',
    });
    commands.importReadText.mockResolvedValue(ok(list(SONGS)));
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.type(within(sheet).getByRole('textbox'), 'https://open.spotify.com/playlist/x');
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    expect(await within(sheet).findByTestId('source-issue')).toHaveTextContent('Issue with source');

    await user.click(within(sheet).getByRole('tab', { name: 'Text or CSV' }));
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Songs, one per line' }),
      'Tame Impala - Loser',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    expect(commands.importReadText).toHaveBeenCalledWith('Tame Impala - Loser', null);
    expect(await within(sheet).findByTestId('import-review')).toBeInTheDocument();
  });

  it('offers to download the text reader before reading screenshots', async () => {
    const user = userEvent.setup();
    commands.importReaderReady.mockResolvedValue(false);
    commands.importPrepareReader.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.click(within(sheet).getByRole('tab', { name: 'Screenshots' }));
    await user.click(
      await within(sheet).findByRole('button', { name: 'Download the text reader (12 MB)' }),
    );
    expect(commands.importPrepareReader).toHaveBeenCalled();
  });
});
