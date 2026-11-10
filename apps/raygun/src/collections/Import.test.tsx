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
  joined: [],
});

const SONGS = [song('Loser', 'Tame Impala'), song('Housewife', 'Tate McRae')];

const candidate = (provider: string, id = provider.toLowerCase()) => ({
  providerId: id,
  providerName: provider,
  points: 90,
  track: {
    title: 'Loser',
    artist: 'Tame Impala',
    album: null,
    durationMs: 200_000,
    artworkUrl: null,
    externalId: null,
    source: null,
  },
});

describe('Importing songs', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.importMatch.mockImplementation(async (songs: ImportedSong[]) =>
      ok(
        songs.map((one): MatchedSong => ({
          song: { ...one, artworkUrl: 'https://img.example/a.jpg' },
          matchedBy: one.title === 'Loser' ? 'Tunes' : null,
          source: null,
          // Two answers from one source (as MusicBrainz gives): only the first is in use.
          candidates:
            one.title === 'Loser'
              ? [candidate('Tunes'), candidate('Tunes', 'tunes-2'), candidate('Indie Tracks')]
              : [],
          pending: false,
        })),
      ),
    );
  });

  it('reads a playlist link, matches the songs and makes a playlist', async () => {
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
    expect(review).toHaveTextContent("The link shares only a playlist's first 100 songs");
    expect(within(review).getByLabelText('Found on Tunes')).toBeInTheDocument();
    expect(within(review).getByLabelText('Unavailable for now')).toBeInTheDocument();

    // Untick one, rename, and save.
    await user.click(within(review).getByRole('checkbox', { name: 'Keep Housewife' }));
    const name = within(review).getByRole('textbox', { name: 'Playlist name' });
    await user.clear(name);
    await user.type(name, 'Hits');
    await user.click(within(review).getByRole('button', { name: 'Make a playlist of 1 song' }));
    expect(commands.importSave).toHaveBeenCalledWith(
      { kind: 'new', name: 'Hits' },
      [
        expect.objectContaining({
          song: expect.objectContaining({ title: 'Loser' }),
          matchedBy: 'Tunes',
        }),
      ],
      false,
    );
  });

  it('fixes names, adds and moves songs, picks another source, and imports while matching', async () => {
    const user = userEvent.setup();
    commands.importReadLink.mockResolvedValue(ok(list(SONGS, false)));
    commands.importSave.mockResolvedValue(ok('p-new'));
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Playlist, album or song link' }),
      'https://open.spotify.com/album/x',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    const review = await within(sheet).findByTestId('import-review');
    await within(review).findByLabelText('Found on Tunes');

    // Another source for Loser.
    await user.click(within(review).getByRole('button', { name: '3 sources' }));
    const sources = within(review).getByRole('list', { name: 'Sources for Loser' });
    expect(within(sources).getAllByRole('button', { name: 'In use' })).toHaveLength(1);
    await user.click(
      within(sources).getAllByRole('button', { name: 'Use this' })[1] as HTMLElement,
    );
    expect(within(sources).getAllByRole('button', { name: 'In use' })).toHaveLength(1);
    expect(within(review).getByLabelText('Found on Indie Tracks')).toBeInTheDocument();

    // From here matching never answers: importing doesn't wait for it.
    commands.importMatch.mockImplementation(() => new Promise(() => undefined));
    // A fixed name, an added song, and Housewife moved up.
    const titles = within(review).getAllByRole('textbox', { name: 'Song title' });
    await user.clear(titles[1] as HTMLElement);
    await user.type(titles[1] as HTMLElement, 'Housewife (Live)');
    await user.type(within(review).getByRole('textbox', { name: 'Add a song: title' }), 'New One');
    await user.click(within(review).getByRole('button', { name: 'Add the song' }));
    await user.click(within(review).getByRole('button', { name: 'Move Housewife (Live) up' }));

    // Imported at once; what isn't matched yet is matched by Rust afterwards.
    await user.click(within(review).getByRole('button', { name: 'Make a playlist of 3 songs' }));
    expect(commands.importSave).toHaveBeenCalledWith(
      { kind: 'new', name: "Today's Top Hits" },
      [
        expect.objectContaining({ song: expect.objectContaining({ title: 'Housewife (Live)' }) }),
        expect.objectContaining({ matchedBy: 'Indie Tracks' }),
        expect.objectContaining({
          song: expect.objectContaining({ title: 'New One' }),
          pending: true,
        }),
      ],
      false,
    );
  });

  it('marks songs a playlist has already and asks before adding them again', async () => {
    const user = userEvent.setup();
    commands.importReadLink.mockResolvedValue(ok(list(SONGS)));
    commands.importSave.mockResolvedValue(ok('p1'));
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road', itemCount: 1, artworkUrls: [], coverUrl: null }]),
    );
    commands.collectionsSavedIndex.mockResolvedValue(
      ok([
        {
          trackKey: 'tame impala|loser',
          songKeys: ['tame impala|loser'],
          favorite: false,
          playlistIds: ['p1'],
        },
      ]),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Playlist, album or song link' }),
      'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    const review = await within(sheet).findByTestId('import-review');
    await user.selectOptions(within(review).getByRole('combobox'), 'p1');
    expect(await within(review).findByText('Already in Road')).toBeInTheDocument();

    await user.click(within(review).getByRole('button', { name: 'Add 2 songs to Road' }));
    expect(within(review).getByRole('alertdialog')).toHaveTextContent('1 song is already in Road');
    expect(commands.importSave).not.toHaveBeenCalled();
    await user.click(within(review).getByRole('button', { name: 'Add new ones' }));
    expect(commands.importSave).toHaveBeenCalledWith(
      { kind: 'playlist', id: 'p1' },
      expect.any(Array),
      false,
    );
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
    expect(await within(sheet).findByTestId('source-issue')).toHaveTextContent(
      'Spotify has no such list; it may be private or removed',
    );

    await user.click(within(sheet).getByRole('tab', { name: 'Text or CSV' }));
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Songs, one per line' }),
      'Tame Impala - Loser',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read songs' }));
    expect(commands.importReadText).toHaveBeenCalledWith('Tame Impala - Loser', null);
    expect(await within(sheet).findByTestId('import-review')).toBeInTheDocument();
  });

  it('reads screenshots joining split rows, and splits a joined row back', async () => {
    const user = userEvent.setup();
    const titleOnly = (title: string): ImportedSong => ({ ...song(title, ''), artist: null });
    commands.importReadScreenshots.mockResolvedValue(
      ok({
        ...list([song('Espresso', 'Sabrina Carpenter'), song('Loser', 'Tame Impala')]),
        joined: [{ index: 0, parts: [titleOnly('Espresso'), titleOnly('Sabrina Carpenter')] }],
      }),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.click(within(sheet).getByRole('tab', { name: 'Screenshots' }));
    await user.upload(
      within(sheet).getByLabelText('Choose screenshots'),
      new File(['png'], 'shot.png', { type: 'image/png' }),
    );
    const join = within(sheet).getByRole('checkbox', { name: 'Join split titles and artists' });
    expect(join).toBeChecked();
    await user.click(within(sheet).getByRole('button', { name: 'Read 1 screenshot' }));
    expect(commands.importReadScreenshots).toHaveBeenCalledWith(['upload-1'], true);

    const review = await within(sheet).findByTestId('import-review');
    expect(within(review).getAllByText('Joined')).toHaveLength(1);
    await user.click(
      within(review).getByRole('button', { name: 'Split Espresso back into two songs' }),
    );
    expect(within(review).queryByText('Joined')).not.toBeInTheDocument();
    const titles = within(review)
      .getAllByRole('textbox', { name: 'Song title' })
      .map((field) => (field as HTMLInputElement).value);
    expect(titles).toEqual(['Espresso', 'Sabrina Carpenter', 'Loser']);
  });

  it('reads screenshots as read when joining is off', async () => {
    const user = userEvent.setup();
    commands.importReadScreenshots.mockResolvedValue(ok(list(SONGS)));
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.click(within(sheet).getByRole('tab', { name: 'Screenshots' }));
    await user.upload(
      within(sheet).getByLabelText('Choose screenshots'),
      new File(['png'], 'shot.png', { type: 'image/png' }),
    );
    await user.click(
      within(sheet).getByRole('checkbox', { name: 'Join split titles and artists' }),
    );
    await user.click(within(sheet).getByRole('button', { name: 'Read 1 screenshot' }));
    expect(commands.importReadScreenshots).toHaveBeenCalledWith(['upload-1'], false);
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

  it('cancels the text reader’s download, offering it again', async () => {
    const user = userEvent.setup();
    commands.importReaderReady.mockResolvedValue(false);
    commands.readerAiSupport.mockResolvedValue({
      textReaderBuiltIn: false,
      textReaderModel: 'ocrs-latin',
    });
    // Rust answers once the download stopped, as if done: the reader is still missing.
    let stopped: () => void = () => {};
    commands.importPrepareReader.mockReturnValue(
      new Promise((resolve) => (stopped = () => resolve(ok(null)))),
    );
    commands.aiModelCancel.mockImplementation(async () => {
      stopped();
      return true;
    });
    await AppShellWrapper.mount('/library?view=playlists');
    await user.click(await screen.findByRole('button', { name: 'Import songs' }));
    const sheet = await screen.findByRole('dialog', { name: 'Import songs' });
    await user.click(within(sheet).getByRole('tab', { name: 'Screenshots' }));
    await user.click(
      await within(sheet).findByRole('button', { name: 'Download the text reader (12 MB)' }),
    );
    expect(
      within(sheet).getByRole('button', { name: 'Downloading the text reader…' }),
    ).toBeDisabled();

    await user.click(await within(sheet).findByRole('button', { name: 'Cancel' }));
    expect(commands.aiModelCancel).toHaveBeenCalledWith('ocrs-latin');
    expect(
      await within(sheet).findByRole('button', { name: 'Download the text reader (12 MB)' }),
    ).toBeEnabled();
    expect(within(sheet).queryByRole('button', { name: 'Cancel' })).toBeNull();
  });
});
