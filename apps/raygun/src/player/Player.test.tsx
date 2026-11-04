import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second'), makeItem('Third')];

describe('Player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows what is playing in the mini player', async () => {
    await AppShellWrapper.mount();

    AppShellWrapper.playing(QUEUE, 0);

    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Intro');
    expect(AppShellWrapper.miniPlayer.element).toHaveTextContent('Raygun');
  });

  it('toggles playback from the mini player', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0, 'paused');
    await AppShellWrapper.miniPlayer.title;

    await AppShellWrapper.miniPlayer.togglePlayback('Play');

    expect(bindingsMock.commands.playbackControl).toHaveBeenCalledWith({ type: 'toggle' });
  });

  it('shows position, duration and the up-next list in the sheet', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();

    const sheet = await AppShellWrapper.playerSheet.element;

    expect(sheet).toHaveTextContent('Intro');
    expect(sheet).toHaveTextContent('1:05');
    expect(sheet).toHaveTextContent('3:00');
    // Up next is a view of its own, so the controls never move when the queue changes.
    expect(screen.queryByTestId('up-next')).not.toBeInTheDocument();
    await AppShellWrapper.playerSheet.press('Up next');
    const upNext = await AppShellWrapper.playerSheet.upNext;
    expect(upNext).toHaveTextContent('Second');
    expect(upNext).toHaveTextContent('Third');
  });

  it('moves on in up next when only the playing item changes', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.press('Up next');
    const { emitEvent, IDLE_STATE } = await import('../test/bindingsMock');

    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: 'id-Second',
    });

    const upNext = await AppShellWrapper.playerSheet.upNext;
    expect(upNext).not.toHaveTextContent('Second');
    expect(upNext).toHaveTextContent('Third');
  });

  it('sends skip, repeat and shuffle controls', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 1);
    await AppShellWrapper.miniPlayer.open();

    await AppShellWrapper.playerSheet.press('Next');
    await AppShellWrapper.playerSheet.press('Previous');
    await AppShellWrapper.playerSheet.press('Repeat off');
    await AppShellWrapper.playerSheet.press('Shuffle');

    const sent = bindingsMock.commands.playbackControl.mock.calls.map(([control]) => control);
    expect(sent).toEqual([
      { type: 'next' },
      { type: 'previous' },
      { type: 'setRepeat', mode: 'all' },
      { type: 'setShuffle', enabled: true },
    ]);
  });

  it('jumps to an item from the up-next list', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.press('Up next');

    await AppShellWrapper.playerSheet.press('Third');

    expect(bindingsMock.commands.playbackControl).toHaveBeenCalledWith({
      type: 'playItem',
      itemId: 'id-Third',
    });
  });

  it('shows playback errors', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    const { emitEvent, IDLE_STATE } = await import('../test/bindingsMock');

    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'error',
      currentItemId: 'id-Intro',
      error: 'A network error interrupted playback',
    });

    expect(await AppShellWrapper.playerSheet.element).toHaveTextContent(
      'A network error interrupted playback',
    );
  });
});

describe('Player with resolved streams', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('says a source is being found, then where it came from', async () => {
    const { emitEvent, IDLE_STATE } = await import('../test/bindingsMock');
    const song = {
      ...makeItem('What a Wonderful World', 'Louis Armstrong'),
      source: {
        kind: 'lookup' as const,
        title: 'What a Wonderful World',
        artist: 'Louis Armstrong',
        album: null,
        durationMs: 137_000,
      },
    };
    await AppShellWrapper.mount();
    emitEvent('queueEvent', { items: [song], order: [0], context: null });
    emitEvent('playbackStateEvent', { ...IDLE_STATE, status: 'loading', currentItemId: song.id });

    // The bar shows up with the queue, then says it's finding a source.
    expect(
      await within(await screen.findByTestId('mini-player')).findByTestId('source-note'),
    ).toHaveTextContent('Finding a source…');

    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: song.id,
      sourceLabel: 'Internet Archive',
    });
    expect(
      await within(AppShellWrapper.miniPlayer.element).findByText('Louis Armstrong'),
    ).toBeInTheDocument();
    await AppShellWrapper.miniPlayer.open();
    expect(
      await within(await AppShellWrapper.playerSheet.element).findByTestId('sheet-source-note'),
    ).toHaveTextContent('via Internet Archive');
  });
});

describe('Player with video', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('plays YouTube songs as music, and shows their video only when asked', async () => {
    const video = {
      ...makeItem('Clip'),
      mediaKind: 'video' as const,
      source: { kind: 'youTube' as const, videoId: 'M7lc1UVf-VE' },
    };
    await AppShellWrapper.mount();
    AppShellWrapper.playing([video], 0);
    await AppShellWrapper.miniPlayer.open();

    const sheet = await AppShellWrapper.playerSheet.element;

    expect(within(sheet).queryByTestId('video-surface')).not.toBeInTheDocument();
    await userEvent.click(within(sheet).getAllByRole('button', { name: 'Show the video' })[0]!);
    expect(within(sheet).getByTestId('video-surface')).toBeInTheDocument();
    // The song moves to YouTube's embed, which has the picture (M46.51: else its audio file).
    expect(bindingsMock.commands.playbackSetVideo).toHaveBeenLastCalledWith(true);
    await userEvent.click(within(sheet).getByRole('button', { name: 'Show artwork' }));
    expect(within(sheet).queryByTestId('video-surface')).not.toBeInTheDocument();
    expect(bindingsMock.commands.playbackSetVideo).toHaveBeenLastCalledWith(false);
  });
});

describe('Lyrics', () => {
  const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  const openLyrics = async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await userEvent.click(await screen.findByRole('button', { name: 'Lyrics' }));
    return screen.findByTestId('lyrics-view');
  };

  it('highlights the current synced line and seeks when a line is tapped', async () => {
    bindingsMock.commands.lyricsGet.mockResolvedValue(
      ok({
        providerName: 'LRCLIB',
        synced: [
          { timeMs: 1_000, text: 'First line' },
          { timeMs: 60_000, text: 'Second line' },
          { timeMs: 120_000, text: 'Third line' },
        ],
        plain: null,
      }),
    );
    const view = await openLyrics();

    // The wrapper's playing state sits at 1:05, inside the second line.
    const current = await within(view).findByRole('button', { current: true });
    expect(current).toHaveTextContent('Second line');
    expect(bindingsMock.commands.lyricsGet).toHaveBeenCalledWith({
      title: 'Intro',
      artist: 'Raygun',
      album: null,
      durationMs: 180_000,
    });
    expect(view).toHaveTextContent('Lyrics from LRCLIB');

    await userEvent.click(within(view).getByRole('button', { name: 'Third line' }));
    expect(bindingsMock.commands.playbackControl).toHaveBeenCalledWith({
      type: 'seek',
      positionMs: 120_000,
    });

    // Back to the artwork.
    await userEvent.click(screen.getByRole('button', { name: 'Show artwork' }));
    expect(screen.queryByTestId('lyrics-view')).not.toBeInTheDocument();
  });

  it('shows plain lyrics when there are no timings', async () => {
    bindingsMock.commands.lyricsGet.mockResolvedValue(
      ok({
        providerName: 'LRCLIB',
        synced: [],
        plain: `Verse one${String.fromCharCode(10)}Verse two`,
      }),
    );
    const view = await openLyrics();
    expect(await within(view).findByText(/Verse one/)).toBeInTheDocument();
  });

  it('says when there are no lyrics or they failed to load', async () => {
    const view = await openLyrics();
    expect(await within(view).findByText('No lyrics found for this track.')).toBeInTheDocument();
  });

  it('shows why lyrics failed', async () => {
    bindingsMock.commands.lyricsGet.mockResolvedValue({
      status: 'error',
      error: 'LRCLIB answered HTTP 502',
    });
    const view = await openLyrics();
    expect(await within(view).findByRole('alert')).toHaveTextContent('LRCLIB answered HTTP 502');
  });
});

describe('Cards under the player', () => {
  const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  const openPlayer = async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    return AppShellWrapper.playerSheet.element;
  };

  it('shows the lyrics, the artist and the credits, each opening more', async () => {
    bindingsMock.commands.lyricsGet.mockResolvedValue(
      ok({
        providerName: 'LRCLIB',
        synced: [
          { timeMs: 1_000, text: 'First line' },
          { timeMs: 60_000, text: 'Second line' },
          { timeMs: 120_000, text: 'Third line' },
        ],
        plain: null,
      }),
    );
    bindingsMock.commands.musicArtistInfo.mockResolvedValue(
      ok({
        name: 'Raygun',
        kind: 'Group',
        bio: 'A band from the tests.',
        imageUrl: null,
        url: 'https://en.wikipedia.org/wiki/Raygun',
        origin: 'Detroit',
        active: '1999–',
        members: [],
        providerName: 'MusicBrainz',
      }),
    );
    bindingsMock.commands.musicSongCredits.mockResolvedValue(
      ok({
        credits: [
          { role: 'Performed by', names: ['Raygun'] },
          { role: 'Produced by', names: ['Someone Else'] },
        ],
        url: 'https://musicbrainz.org/recording/x',
        providerName: 'MusicBrainz',
      }),
    );
    const sheet = await openPlayer();

    // The "Up next" pill names the next song.
    expect(within(sheet).getByRole('button', { name: 'Up next' })).toHaveTextContent('Second');
    // Synced lyrics show from the line playing (the wrapper sits at 1:05).
    const lyrics = await within(sheet).findByTestId('lyrics-card');
    expect(await within(lyrics).findByText('Second line')).toBeInTheDocument();
    expect(lyrics).not.toHaveTextContent('First line');
    const artist = within(sheet).getByTestId('artist-card');
    expect(await within(artist).findByText('A band from the tests.')).toBeInTheDocument();
    expect(artist).toHaveTextContent('Detroit · 1999–');
    expect(bindingsMock.commands.musicArtistInfo).toHaveBeenCalledWith('Raygun');
    const credits = within(sheet).getByTestId('credits-card');
    expect(await within(credits).findByText('Someone Else')).toBeInTheDocument();
    expect(credits).toHaveTextContent('Produced by');

    await userEvent.click(credits);
    const list = await screen.findByTestId('credits-list');
    expect(within(list).getByRole('link', { name: 'Someone Else' })).toBeInTheDocument();
    await userEvent.click(within(list).getByRole('button', { name: 'Open on MusicBrainz' }));
    expect(bindingsMock.commands.musicOpenInfoPage).toHaveBeenCalledWith(
      'https://musicbrainz.org/recording/x',
    );
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));

    await userEvent.click(within(sheet).getByRole('button', { name: 'Lyrics' }));
    expect(await within(sheet).findByTestId('lyrics-view')).toBeInTheDocument();
    expect(within(sheet).queryByTestId('player-cards')).not.toBeInTheDocument();
  });

  it('credits the song to its artists when no source knows it', async () => {
    const sheet = await openPlayer();
    const credits = await within(sheet).findByTestId('credits-card');
    await waitFor(() => expect(credits).toHaveTextContent('Performed by'));
    expect(credits).toHaveTextContent('Raygun');
    const artist = within(sheet).getByTestId('artist-card');
    expect(await within(artist).findByText('See their songs and albums')).toBeInTheDocument();
  });

  it('leads to the artist page and closes the player', async () => {
    const sheet = await openPlayer();
    await userEvent.click(within(sheet).getByRole('button', { name: 'About Raygun' }));
    expect(await screen.findByTestId('music-artist-page')).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Now playing' })).not.toBeInTheDocument(),
    );
  });
});
