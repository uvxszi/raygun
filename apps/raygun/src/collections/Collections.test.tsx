import type { OnlineTrack, PlaylistItem, SavedTrack } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { useTrackActionsStore } from './trackActionsStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const saved = (title: string, artworkUrl: string | null = null): SavedTrack => ({
  trackKey: `nina simone|${title.toLowerCase()}`,
  title,
  artist: 'Nina Simone',
  album: null,
  durationMs: 180_000,
  artworkUrl,
  source: null,
  local: false,
  addedAt: null,
  playCount: 0,
});

const online: OnlineTrack = {
  externalId: 'mbid-1',
  title: 'Feeling Good',
  artist: 'Nina Simone',
  album: 'I Put a Spell on You',
  durationMs: 170_000,
  artworkUrl: null,
  source: null,
};

const PROVIDER = {
  id: 'dev.raygun.musicbrainz/recordings',
  extensionId: 'dev.raygun.musicbrainz',
  enabled: true,
  kind: 'music.metadata',
  name: 'MusicBrainz',
};

const ITEMS: PlaylistItem[] = [
  { id: 'i1', track: saved('One') },
  { id: 'i2', track: saved('Two') },
  { id: 'i3', track: saved('Three') },
];

const showPlaylist = () =>
  commands.playlistGet.mockResolvedValue(
    ok({
      playlist: { id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] },
      items: ITEMS,
    }),
  );

describe('Favorites and playlists', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('favorites a search result from its action sheet', async () => {
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: PROVIDER, tracks: [online], error: null }],
      }),
    );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=feeling+good');

    await userEvent.click(await screen.findByRole('button', { name: 'More for Feeling Good' }));
    const sheet = await screen.findByRole('dialog', { name: 'Feeling Good' });
    await userEvent.click(within(sheet).getByRole('button', { name: 'Add to favorites' }));

    expect(commands.favoriteSet).toHaveBeenCalledWith(
      {
        title: 'Feeling Good',
        artist: 'Nina Simone',
        album: 'I Put a Spell on You',
        durationMs: 170_000,
        artworkUrl: null,
        source: null,
      },
      true,
    );
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('adds a track to a new playlist', async () => {
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: PROVIDER, tracks: [online], error: null }],
      }),
    );
    commands.playlistCreate.mockResolvedValue(
      ok({ id: 'p9', name: 'Road trip', itemCount: 0, artworkUrls: [] }),
    );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=feeling+good');

    await userEvent.click(await screen.findByRole('button', { name: 'More for Feeling Good' }));
    const sheet = await screen.findByRole('dialog');
    await userEvent.click(within(sheet).getByRole('button', { name: 'New playlist' }));
    await userEvent.type(within(sheet).getByLabelText('Playlist name'), 'Road trip');
    await userEvent.click(within(sheet).getByRole('button', { name: 'Create' }));

    expect(commands.playlistCreate).toHaveBeenCalledWith('Road trip');
    await vi.waitFor(() =>
      expect(commands.playlistAdd).toHaveBeenCalledWith(
        'p9',
        [expect.objectContaining({ title: 'Feeling Good' })],
        false,
      ),
    );
  });

  it('knows an imported song on other screens and asks before adding it again', async () => {
    // Imported from Spotify with every artist credited; a source lists the main artist only.
    commands.collectionsSavedIndex.mockResolvedValue(
      ok([
        {
          trackKey: 'daft punk pharrell williams|get lucky',
          songKeys: ['daft punk pharrell williams|get lucky'],
          favorite: false,
          playlistIds: ['p1'],
        },
      ]),
    );
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [], coverUrl: null }]),
    );
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: PROVIDER,
            tracks: [{ ...online, title: 'Get Lucky', artist: 'Daft Punk', album: null }],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=get+lucky');

    // The saved mark, under the source's names.
    expect(await screen.findByRole('img', { name: 'Saved' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'More for Get Lucky' }));
    const sheet = await screen.findByRole('dialog', { name: 'Get Lucky' });
    const road = await within(sheet).findByRole('button', { name: /Road trip/ });
    expect(within(road).getByRole('img', { name: 'Saved in this playlist' })).toBeInTheDocument();

    // Adding it again asks first, like Spotify; Add anyway adds a second copy on purpose.
    await userEvent.click(road);
    expect(within(sheet).getByRole('alertdialog')).toHaveTextContent('Already in Road trip');
    expect(commands.playlistAdd).not.toHaveBeenCalled();
    await userEvent.click(within(sheet).getByRole('button', { name: 'Add anyway' }));
    expect(commands.playlistAdd).toHaveBeenCalledWith(
      'p1',
      [expect.objectContaining({ title: 'Get Lucky', artist: 'Daft Punk' })],
      true,
    );
  });

  it('asks when Rust finds the song already there under other names', async () => {
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [], coverUrl: null }]),
    );
    commands.playlistAdd.mockResolvedValue(ok({ added: 0, already: 1 }));
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: PROVIDER, tracks: [online], error: null }],
      }),
    );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=feeling+good');
    await userEvent.click(await screen.findByRole('button', { name: 'More for Feeling Good' }));
    const sheet = await screen.findByRole('dialog', { name: 'Feeling Good' });
    await userEvent.click(await within(sheet).findByRole('button', { name: /Road trip/ }));
    expect(commands.playlistAdd).toHaveBeenCalledWith('p1', [expect.anything()], false);
    expect(await within(sheet).findByRole('alertdialog')).toHaveTextContent('Already in Road trip');
    await userEvent.click(within(sheet).getByRole('button', { name: "Don't add" }));
    expect(within(sheet).queryByRole('alertdialog')).not.toBeInTheDocument();
    act(() => useTrackActionsStore.getState().close());
  });

  it('plays a playlist, from a track, and reorders and removes items while editing', async () => {
    showPlaylist();
    await AppShellWrapper.mount('/library/playlist?id=p1');
    const page = await screen.findByTestId('playlist-page');
    expect(await within(page).findByRole('heading', { name: 'Road trip' })).toBeInTheDocument();

    await userEvent.click(within(page).getByRole('button', { name: 'Play' }));
    expect(commands.collectionPlay).toHaveBeenLastCalledWith({
      collection: { kind: 'playlist', id: 'p1' },
      startIndex: null,
      shuffle: false,
    });
    // A song plays the table from it, in the order shown (M46.8).
    const row = within(page).getAllByTestId('track-table-row')[1] as HTMLElement;
    await userEvent.click(within(row).getByRole('button', { name: 'Two' }));
    expect(commands.collectionPlay).toHaveBeenLastCalledWith({
      collection: { kind: 'playlist', id: 'p1' },
      startIndex: 1,
      shuffle: false,
      order: ['nina simone|one', 'nina simone|two', 'nina simone|three'],
    });

    // Rows have no grips outside edit mode (M46.49).
    expect(
      within(page).queryByRole('button', { name: 'Drag to move One' }),
    ).not.toBeInTheDocument();
    // Edit opens its options; edit mode is one of them (M46.46), shared with the table's button.
    await userEvent.click(within(page).getByRole('button', { name: 'Edit' }));
    const options = await screen.findByTestId('playlist-edit');
    // A sheet of its own on the page's body, never inside the scrolling list (M46.49).
    expect(within(page).queryByTestId('playlist-edit')).not.toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Edit playlist' })).toContainElement(options);
    await userEvent.click(within(options).getByRole('switch', { name: 'Edit songs' }));
    await userEvent.keyboard('{Escape}');
    expect(within(page).getByRole('button', { name: 'Done' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(within(page).getByRole('button', { name: 'Drag to move One' })).toBeInTheDocument();
    expect(within(page).getByRole('button', { name: 'Move One up' })).toBeDisabled();
    await userEvent.click(within(page).getByRole('button', { name: 'Move One down' }));
    expect(commands.playlistMoveItem).toHaveBeenCalledWith('i1', 'i3');
    await userEvent.click(within(page).getByRole('button', { name: 'Move Three up' }));
    expect(commands.playlistMoveItem).toHaveBeenLastCalledWith('i3', 'i2');
    await userEvent.click(within(page).getByRole('button', { name: 'Remove Two' }));
    expect(commands.playlistRemoveItem).toHaveBeenCalledWith('i2');
  });

  it('favorites a selection in one call, or takes it out when all are favorites', async () => {
    showPlaylist();
    commands.collectionsSavedIndex.mockResolvedValue(
      ok(
        ['one', 'two', 'three'].map((title) => ({
          trackKey: `nina simone|${title}`,
          songKeys: [`nina simone|${title}`],
          favorite: title !== 'three',
          playlistIds: ['p1'],
        })),
      ),
    );
    await AppShellWrapper.mount('/library/playlist?id=p1');
    const page = await screen.findByTestId('playlist-page');
    await userEvent.click(within(page).getByRole('button', { name: 'Select all' }));
    const bar = within(page).getByRole('toolbar', { name: 'Selected songs' });
    await userEvent.click(within(bar).getByRole('button', { name: 'Favorite' }));
    expect(commands.favoritesSetMany).toHaveBeenCalledWith(
      [
        expect.objectContaining({ title: 'One' }),
        expect.objectContaining({ title: 'Two' }),
        expect.objectContaining({ title: 'Three' }),
      ],
      true,
    );

    // Two picked, both favorites already: the bar offers to take them out.
    await userEvent.click(within(bar).getByRole('button', { name: 'Clear the selection' }));
    const rows = within(page).getAllByTestId('track-table-row');
    const user = userEvent.setup();
    await user.click(within(rows[0] as HTMLElement).getByRole('checkbox', { name: 'Select One' }));
    await user.keyboard('{Control>}');
    await user.click(rows[1] as HTMLElement);
    await user.keyboard('{/Control}');
    const picked = within(page).getByRole('toolbar', { name: 'Selected songs' });
    await userEvent.click(within(picked).getByRole('button', { name: 'Remove from favorites' }));
    expect(commands.favoritesSetMany).toHaveBeenLastCalledWith(
      [expect.objectContaining({ title: 'One' }), expect.objectContaining({ title: 'Two' })],
      false,
    );
  });

  it('marks the explicit songs among favorites', async () => {
    commands.favoritesList.mockResolvedValue(
      ok([{ ...saved('Sinnerman'), explicit: true }, saved('Feeling Good')]),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    await userEvent.click(await screen.findByTestId('library-view-favorites'));
    expect(await screen.findByText('Feeling Good')).toBeInTheDocument();
    expect(screen.getAllByRole('img', { name: 'Explicit' })).toHaveLength(1);
  });

  it('lists playlists and favorites in the library without any folders', async () => {
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
    commands.favoritesList.mockResolvedValue(ok([saved('Sinnerman')]));
    await AppShellWrapper.mount('/library?view=playlists');

    expect(await screen.findByTestId('playlist-link')).toHaveTextContent('Road trip3 tracks');
    expect(screen.queryByText(/find the music already on this device/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId('library-view-favorites'));
    await userEvent.click(await screen.findByRole('button', { name: 'Shuffle' }));
    expect(commands.collectionPlay).toHaveBeenCalledWith({
      collection: { kind: 'favorites' },
      startIndex: null,
      shuffle: true,
    });
  });

  it('shows recently played on Home and plays history from a track', async () => {
    commands.historyRecent.mockResolvedValue(
      ok([
        { track: saved('Sinnerman'), playedAt: 2 },
        { track: saved('Feeling Good'), playedAt: 1 },
      ]),
    );
    await AppShellWrapper.mount('/');

    const recent = await screen.findByTestId('recently-played');
    const rows = await within(recent).findAllByTestId('saved-track-row');
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining('Sinnerman'),
      expect.stringContaining('Feeling Good'),
    ]);
    await userEvent.click(rows[1] as HTMLElement);
    expect(commands.collectionPlay).toHaveBeenCalledWith({
      collection: { kind: 'history' },
      startIndex: 1,
      shuffle: false,
    });
    expect(commands.historyRecent).toHaveBeenCalledWith(10);
  });

  it('saves the current song with the Plus: Favorites at once, then playlists', async () => {
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road', itemCount: 3, artworkUrls: [] }]),
    );
    await AppShellWrapper.mount('/');
    AppShellWrapper.playing([makeItem('Sinnerman', 'Nina Simone')], 0);
    await AppShellWrapper.miniPlayer.open();

    // The player bar has a Plus too; this one is the sheet's.
    const sheet = await AppShellWrapper.playerSheet.element;
    const plus = await within(sheet).findByRole('button', {
      name: 'Save to Favorites or a playlist',
    });
    await vi.waitFor(() => expect(plus).toBeEnabled());
    await userEvent.click(plus);
    const song = expect.objectContaining({ title: 'Sinnerman', artist: 'Nina Simone' });
    expect(commands.collectionsSetSaves).toHaveBeenCalledWith(song, {
      favorite: true,
      playlistIds: [],
    });

    // The popover: Favorites ticked, a playlist ticked, Done saves both.
    const popover = await screen.findByRole('dialog', { name: 'Add to playlist' });
    expect(within(popover).getByRole('checkbox', { name: 'Favorites' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await userEvent.click(await within(popover).findByRole('checkbox', { name: 'Road' }));
    await userEvent.click(within(popover).getByRole('button', { name: 'Done' }));
    expect(commands.collectionsSetSaves).toHaveBeenLastCalledWith(song, {
      favorite: true,
      playlistIds: ['p1'],
    });
  });
});

describe('Search across sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    // These tests look at each source's own list; best matches have their own tests.
    localStorage.setItem('raygun.search.grouped.music', '1');
  });

  it('shows your library and merged top results above each source', async () => {
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [
          {
            id: 'local-1',
            title: 'Feeling Good',
            artist: 'Nina Simone',
            albumArtist: 'Nina Simone',
            album: 'Pastel Blues',
            trackNo: 1,
            discNo: null,
            year: 1965,
            durationMs: 170_000,
            artworkUrl: null,
          },
        ],
        top: [online],
        providers: [{ provider: PROVIDER, tracks: [online], error: null }],
      }),
    );
    await AppShellWrapper.mount('/search?type=music&view=songs&q=feeling+good');

    const headings = await screen.findAllByRole('heading', { level: 3 });
    expect(headings.map((heading) => heading.textContent)).toEqual([
      'Your library',
      'Top results',
      'MusicBrainz',
    ]);
    await userEvent.click(within(screen.getByTestId('library-results')).getByTestId('track-row'));
    expect(commands.libraryPlay).toHaveBeenCalledWith({
      selection: { kind: 'allTracks' },
      startTrackId: 'local-1',
      shuffle: false,
    });
  });
});
