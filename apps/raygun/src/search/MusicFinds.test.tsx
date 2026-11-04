import type { MusicAlbum, OnlineTrack } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const TUNES = {
  id: 'dev.example.tunes/catalogue',
  extensionId: 'dev.example.tunes',
  kind: 'music.metadata',
  name: 'Tunes',
};
const MUSICBRAINZ = { ...TUNES, id: 'dev.raygun.musicbrainz/recordings', name: 'MusicBrainz' };

const song = (title: string, artist: string): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album: null,
  durationMs: null,
  artworkUrl: null,
  source: null,
});

const ALBUM: MusicAlbum = {
  provider: TUNES,
  id: '6575789',
  title: 'Random Access Memories',
  artist: 'Daft Punk',
  year: 2013,
  artworkUrl: null,
  trackCount: 13,
};

describe('Music search: songs, artists and albums', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    localStorage.clear();
  });

  it('lists the best matching songs from every source, naming each source', async () => {
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          { provider: MUSICBRAINZ, tracks: [song('Lucky Star', 'Madonna')], error: null },
          { provider: TUNES, tracks: [song('Get Lucky', 'Daft Punk')], error: null },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get%20lucky');
    const best = await screen.findByTestId('best-songs');
    const rows = within(best).getAllByTestId('online-track-row');
    expect(rows[0]).toHaveTextContent('Get Lucky');
    expect(rows[0]).toHaveTextContent('Tunes');
    expect(rows[1]).toHaveTextContent('Lucky Star');
  });

  it('marks explicit songs and 18+ sources, and queues the explicit mark', async () => {
    const user = userEvent.setup();
    const ADULT = { ...TUNES, id: 'dev.test.adult/songs', name: 'After Dark', adult: true };
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: ADULT,
            tracks: [{ ...song('Get Lucky', 'Daft Punk'), explicit: true }],
            error: null,
          },
          { provider: MUSICBRAINZ, tracks: [song('Lucky Star', 'Madonna')], error: null },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get%20lucky');
    const best = await screen.findByTestId('best-songs');
    const [explicit, clean] = within(best).getAllByTestId('online-track-row');
    expect(within(explicit!).getByRole('img', { name: 'Explicit' })).toBeInTheDocument();
    expect(within(explicit!).getByRole('img', { name: 'For adults (18+)' })).toBeInTheDocument();
    expect(within(clean!).queryByRole('img', { name: 'Explicit' })).not.toBeInTheDocument();
    expect(within(clean!).queryByRole('img', { name: 'For adults (18+)' })).not.toBeInTheDocument();
    await user.click(explicit!);
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      expect.arrayContaining([expect.objectContaining({ title: 'Get Lucky', explicit: true })]),
      0,
    );
  });

  it('finds artists and albums, and opens an album to play it', async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
    commands.musicSearchArtists.mockResolvedValue(
      ok([
        { provider: TUNES, id: '27', name: 'Daft Punk', imageUrl: null },
        { provider: MUSICBRAINZ, id: 'x', name: 'Daft Punk', imageUrl: null },
      ]),
    );
    commands.musicSearchAlbums.mockResolvedValue(ok([ALBUM]));
    commands.musicAlbum.mockResolvedValue(
      ok({
        album: ALBUM,
        tracks: [song('Give Life Back to Music', 'Daft Punk'), song('Get Lucky', 'Daft Punk')],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=daft%20punk');

    await user.click(await screen.findByRole('button', { name: 'Artists' }));
    const artists = await screen.findByTestId('artist-finds');
    expect(within(artists).getAllByRole('link')).toHaveLength(1);
    expect(within(artists).getByRole('link', { name: /Daft Punk/ })).toHaveAttribute(
      'href',
      expect.stringContaining('/artist'),
    );

    await user.click(screen.getByRole('button', { name: 'Albums' }));
    const albums = await screen.findByTestId('album-finds');
    await user.click(within(albums).getByRole('link', { name: /Random Access Memories/ }));
    const page = await screen.findByTestId('music-album-page');
    expect(within(page).getByRole('heading', { level: 1 })).toHaveTextContent(
      'Random Access Memories',
    );
    expect(commands.musicAlbum).toHaveBeenCalledWith(TUNES.id, '6575789');
    await user.click(within(page).getByRole('button', { name: 'Play all' }));
    expect(commands.playbackReplace).toHaveBeenCalledWith(
      [
        expect.objectContaining({ title: 'Give Life Back to Music' }),
        expect.objectContaining({ title: 'Get Lucky' }),
      ],
      0,
    );
    await user.click(within(page).getByRole('button', { name: 'Download all' }));
    expect(commands.songsDownload).toHaveBeenCalledWith([
      expect.objectContaining({ title: 'Give Life Back to Music' }),
      expect.objectContaining({ title: 'Get Lucky' }),
    ]);
  });

  it('edits an album: its title over what the source said (M46.30)', async () => {
    const user = userEvent.setup();
    commands.musicAlbum.mockResolvedValue(ok({ album: ALBUM, tracks: [] }));
    await AppShellWrapper.mount(`/music/album?provider=${TUNES.id}&id=6575789`);
    await user.click(await screen.findByRole('button', { name: 'Edit album' }));
    const form = await screen.findByTestId('detail-edit');
    const title = within(form).getByRole('textbox', { name: 'Album title' });
    expect(title).toHaveAttribute('placeholder', 'Random Access Memories');
    await user.type(title, 'RAM');
    await user.click(within(form).getByRole('button', { name: 'Save' }));
    expect(commands.detailEditSet).toHaveBeenCalledWith(
      { kind: 'album', artist: 'Daft Punk', album: 'Random Access Memories' },
      { title: 'RAM', artist: null, album: null, artworkUrl: null },
    );
  });

  it('starts on Relevant: the best match of any kind, then songs, artists, albums, podcasts', async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: TUNES, tracks: [song('Get Lucky', 'Daft Punk')], error: null }],
      }),
    );
    commands.musicSearchArtists.mockResolvedValue(
      ok([{ id: '27', name: 'Daft Punk', imageUrl: null, provider: TUNES }]),
    );
    commands.musicSearchAlbums.mockResolvedValue(ok([ALBUM]));
    commands.podcastSearch.mockResolvedValue(
      ok([
        {
          title: 'Daft Punk Talks',
          author: 'Fans',
          feedUrl: 'https://example.com/feed.xml',
          artworkUrl: null,
          description: null,
          subscribed: false,
        },
      ]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=daft%20punk');

    const relevant = await screen.findByTestId('relevant-music');
    expect(screen.getByRole('button', { name: 'Relevant' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // The artist's name answers the query best.
    expect(await within(relevant).findByTestId('top-result')).toHaveTextContent('Daft Punk');
    expect(within(relevant).getByTestId('top-result')).toHaveTextContent('Artist');
    expect(await within(relevant).findByRole('region', { name: 'Albums' })).toHaveTextContent(
      'Random Access Memories',
    );
    expect(await within(relevant).findByRole('region', { name: 'Podcasts' })).toHaveTextContent(
      'Daft Punk Talks',
    );

    // See all moves to that pill.
    const songs = await within(relevant).findByRole('region', { name: 'Songs' });
    await user.click(within(songs).getByRole('button', { name: 'See all' }));
    expect(await screen.findByRole('button', { name: 'Songs' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('mixes in playlists (the services’ own first) and audiobooks, each leading to its pill', async () => {
    const user = userEvent.setup();
    const playlist = (id: string, title: string, featured: boolean) => ({
      provider: TUNES,
      id,
      title,
      author: null,
      artworkUrl: null,
      trackCount: 10,
      featured,
    });
    commands.musicSearchPlaylists.mockResolvedValue(
      ok([playlist('1', 'Daft Punk mix', false), playlist('2', 'Daft Punk essentials', true)]),
    );
    commands.audiobookDirectory.mockResolvedValue(
      ok({
        books: [
          {
            title: 'The Daft Book',
            author: 'Someone',
            feedUrl: 'https://librivox.org/rss/1',
            artworkUrl: null,
            runtime: null,
            language: 'eng',
            description: null,
            source: 'LibriVox',
          },
        ],
        hasMore: false,
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=daft%20punk');
    const relevant = await screen.findByTestId('relevant-music');
    const playlists = await within(relevant).findByRole('region', { name: 'Playlists' });
    expect(
      within(playlists)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual([
      expect.stringContaining('Daft Punk essentials'),
      expect.stringContaining('Daft Punk mix'),
    ]);
    const books = await within(relevant).findByRole('region', { name: 'Audiobooks' });
    expect(within(books).getByText('The Daft Book')).toBeInTheDocument();
    expect(commands.audiobookDirectory).toHaveBeenCalledWith('search', 'daft punk', 1);

    await user.click(within(books).getByRole('button', { name: 'See all' }));
    expect(await screen.findByRole('button', { name: 'Audiobooks' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(await screen.findByTestId('audiobook-search')).toBeInTheDocument();
  });

  it("shows each source's songs as it answers, without waiting for the slowest", async () => {
    let answer: (value: unknown) => void = () => undefined;
    commands.onlineSearch.mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get%20lucky');
    await vi.waitFor(() => expect(commands.onlineSearch).toHaveBeenCalledWith('get lucky'));
    emitEvent('searchProgressEvent', {
      query: 'get lucky',
      total: 2,
      part: {
        kind: 'songs',
        result: { provider: TUNES, tracks: [song('Get Lucky', 'Daft Punk')], error: null },
      },
    });
    expect(await screen.findByText('Get Lucky')).toBeInTheDocument();
    expect(screen.getByText('1 of 2 sources answered')).toBeInTheDocument();

    // Another search's answers don't show here.
    emitEvent('searchProgressEvent', {
      query: 'something else',
      total: 1,
      part: {
        kind: 'songs',
        result: { provider: MUSICBRAINZ, tracks: [song('Elsewhere', 'Nobody')], error: null },
      },
    });
    answer(
      ok({
        library: [],
        top: [],
        providers: [
          { provider: TUNES, tracks: [song('Get Lucky', 'Daft Punk')], error: null },
          { provider: MUSICBRAINZ, tracks: [song('Lucky Star', 'Madonna')], error: null },
        ],
      }),
    );
    expect(await screen.findByText('Lucky Star')).toBeInTheDocument();
    expect(screen.queryByText('Elsewhere')).not.toBeInTheDocument();
    expect(screen.queryByText('1 of 2 sources answered')).not.toBeInTheDocument();
  });

  it("opens podcasts from old links on Music's Podcasts pill", async () => {
    commands.podcastTop.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/browse?tab=search&type=podcast');
    expect(await screen.findByTestId('podcast-search')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Podcasts' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});
