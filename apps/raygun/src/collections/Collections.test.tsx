import type { OnlineTrack, PlaylistItem, SavedTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

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
    await AppShellWrapper.mount('/search?type=music&q=feeling+good');

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
    await AppShellWrapper.mount('/search?type=music&q=feeling+good');

    await userEvent.click(await screen.findByRole('button', { name: 'More for Feeling Good' }));
    const sheet = await screen.findByRole('dialog');
    await userEvent.click(within(sheet).getByRole('button', { name: 'New playlist' }));
    await userEvent.type(within(sheet).getByLabelText('Playlist name'), 'Road trip');
    await userEvent.click(within(sheet).getByRole('button', { name: 'Create' }));

    expect(commands.playlistCreate).toHaveBeenCalledWith('Road trip');
    await vi.waitFor(() =>
      expect(commands.playlistAdd).toHaveBeenCalledWith('p9', [
        expect.objectContaining({ title: 'Feeling Good' }),
      ]),
    );
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
    await userEvent.click(within(page).getAllByTestId('saved-track-row')[1] as HTMLElement);
    expect(commands.collectionPlay).toHaveBeenLastCalledWith({
      collection: { kind: 'playlist', id: 'p1' },
      startIndex: 1,
      shuffle: false,
    });

    await userEvent.click(within(page).getByRole('button', { name: 'Edit' }));
    expect(within(page).getByRole('button', { name: 'Move One up' })).toBeDisabled();
    await userEvent.click(within(page).getByRole('button', { name: 'Move One down' }));
    expect(commands.playlistMoveItem).toHaveBeenCalledWith('i1', 'i3');
    await userEvent.click(within(page).getByRole('button', { name: 'Move Three up' }));
    expect(commands.playlistMoveItem).toHaveBeenLastCalledWith('i3', 'i2');
    await userEvent.click(within(page).getByRole('button', { name: 'Remove Two' }));
    expect(commands.playlistRemoveItem).toHaveBeenCalledWith('i2');
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

  it('hearts the current track in the full player', async () => {
    commands.favoriteGet.mockResolvedValue(ok(true));
    await AppShellWrapper.mount('/');
    AppShellWrapper.playing([makeItem('Sinnerman', 'Nina Simone')], 0);
    await AppShellWrapper.miniPlayer.open();

    // The player bar has a heart too; this one is the sheet's.
    const sheet = await AppShellWrapper.playerSheet.element;
    const heart = await within(sheet).findByRole('button', { name: 'Remove from favorites' });
    expect(heart).toHaveAttribute('aria-pressed', 'true');
    await userEvent.click(heart);
    expect(commands.favoriteSet).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Sinnerman', artist: 'Nina Simone' }),
      false,
    );
  });
});

describe('Search across sources', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
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
    await AppShellWrapper.mount('/search?type=music&q=feeling+good');

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
