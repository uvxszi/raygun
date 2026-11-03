import type { LibraryAlbum, LibraryTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const ALBUM: LibraryAlbum = {
  artist: 'The Raygunners',
  title: 'Pew Pew',
  year: 2026,
  trackCount: 2,
  artworkUrl: 'http://127.0.0.1:9000/art/abc.png',
};

const track = (id: string, title: string, trackNo: number): LibraryTrack => ({
  id,
  title,
  artist: 'The Raygunners',
  albumArtist: 'The Raygunners',
  album: 'Pew Pew',
  trackNo,
  discNo: null,
  year: 2026,
  durationMs: 185_000,
  artworkUrl: null,
});

const TRACKS = [track('t1', 'Laser Beam', 1), track('t2', 'Ray of Light', 2)];

const seedLibrary = () => {
  const { commands } = bindingsMock;
  commands.libraryStatus.mockResolvedValue(
    ok({ roots: ['/storage/emulated/0/Music'], trackCount: 2, scanning: false }),
  );
  commands.libraryAlbums.mockResolvedValue(ok([ALBUM]));
  commands.libraryArtists.mockResolvedValue(
    ok([{ name: 'The Raygunners', albumCount: 1, trackCount: 2, artworkUrl: null }]),
  );
  commands.libraryAlbumTracks.mockResolvedValue(ok(TRACKS));
  commands.libraryTracks.mockResolvedValue(ok(TRACKS));
};

describe('Library', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    // jsdom has no layout; give elements a phone-sized box so the virtualized list renders rows.
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(400);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('offers to find music on the device when there are no folders', async () => {
    await AppShellWrapper.mount('/library?media=music');

    await userEvent.click(await screen.findByRole('button', { name: 'Find music on this device' }));

    expect(bindingsMock.commands.libraryConnectDeviceMusic).toHaveBeenCalled();
  });

  it('shows why connecting failed', async () => {
    bindingsMock.commands.libraryConnectDeviceMusic.mockResolvedValue({
      status: 'error',
      error: 'Raygun needs permission to read your music.',
    });
    await AppShellWrapper.mount('/library?media=music');

    await userEvent.click(await screen.findByRole('button', { name: 'Find music on this device' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('needs permission');
  });

  it('shows everything under All by default and albums with artwork', async () => {
    seedLibrary();
    await AppShellWrapper.mount('/library?media=music');

    const overview = await screen.findByTestId('music-overview');
    expect(screen.getByTestId('library-view-all')).toHaveAttribute('aria-current', 'page');
    const albums = await within(overview).findByRole('region', { name: 'Albums' });
    expect(within(albums).getByRole('link', { name: /Pew Pew/ })).toBeInTheDocument();
    expect(await within(overview).findByRole('region', { name: 'Artists' })).toHaveTextContent(
      'The Raygunners',
    );
    expect(await within(overview).findAllByTestId('track-row')).toHaveLength(2);
    await userEvent.click(within(overview).getByRole('link', { name: 'See all Albums' }));

    const grid = await screen.findByTestId('album-grid');

    expect(grid).toHaveTextContent('Pew Pew');
    expect(screen.getByTestId('library-view-albums')).toHaveAttribute('aria-current', 'page');
    expect(grid).toHaveTextContent('The Raygunners');
    expect(within(grid).getByTestId('artwork-image')).toHaveAttribute('src', ALBUM.artworkUrl);
  });

  it('switches to the artists and tracks views', async () => {
    seedLibrary();
    await AppShellWrapper.mount('/library?media=music');

    await userEvent.click(await screen.findByTestId('library-view-artists'));
    expect(await screen.findByTestId('artist-list')).toHaveTextContent('1 albums · 2 tracks');

    await userEvent.click(screen.getByTestId('library-view-tracks'));
    expect(await screen.findAllByTestId('track-row')).toHaveLength(2);
  });

  it('plays all tracks starting from the tapped one', async () => {
    seedLibrary();
    await AppShellWrapper.mount('/library?view=tracks');

    await userEvent.click((await screen.findAllByTestId('track-row'))[1]!);

    expect(bindingsMock.commands.libraryPlay).toHaveBeenCalledWith({
      selection: { kind: 'allTracks' },
      startTrackId: 't2',
      shuffle: false,
    });
  });

  it('opens an album and plays it from a track or shuffled', async () => {
    seedLibrary();
    await AppShellWrapper.mount('/library?media=music');

    await userEvent.click(await screen.findByRole('link', { name: /Pew Pew/ }));
    const albumPage = await screen.findByTestId('album-page');
    expect(albumPage).toHaveTextContent('2026 · 2 tracks');

    await userEvent.click(
      await within(albumPage).findByRole('button', { name: /^2\s*Ray of Light/ }),
    );
    await userEvent.click(within(albumPage).getByRole('button', { name: 'Shuffle' }));

    const selection = { kind: 'album', artist: 'The Raygunners', album: 'Pew Pew' };
    expect(bindingsMock.commands.libraryPlay.mock.calls.map(([request]) => request)).toEqual([
      { selection, startTrackId: 't2', shuffle: false },
      { selection, startTrackId: null, shuffle: true },
    ]);
  });

  it('shows scan progress while reading files', async () => {
    seedLibrary();
    await AppShellWrapper.mount('/library?view=albums');
    await screen.findByTestId('album-grid');

    emitEvent('libraryScanEvent', {
      phase: 'reading',
      found: 5000,
      toRead: 5000,
      read: 1200,
      removed: 0,
      failed: 0,
      message: null,
    });

    expect(await screen.findByTestId('scan-banner')).toHaveTextContent('Reading 1200 of 5000');
  });
});
