import { within } from '@testing-library/react';

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
    const upNext = await AppShellWrapper.playerSheet.upNext;
    expect(upNext).toHaveTextContent('Second');
    expect(upNext).toHaveTextContent('Third');
  });

  it('moves on in up next when only the playing item changes', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
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
    emitEvent('queueEvent', { items: [song], order: [0] });
    emitEvent('playbackStateEvent', { ...IDLE_STATE, status: 'loading', currentItemId: song.id });

    expect(
      await within(AppShellWrapper.miniPlayer.element).findByTestId('source-note'),
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

  it('shows a video area instead of artwork for YouTube items', async () => {
    const video = {
      ...makeItem('Clip'),
      mediaKind: 'video' as const,
      source: { kind: 'youTube' as const, videoId: 'M7lc1UVf-VE' },
    };
    await AppShellWrapper.mount();
    AppShellWrapper.playing([video], 0);
    await AppShellWrapper.miniPlayer.open();

    const sheet = await AppShellWrapper.playerSheet.element;

    expect(within(sheet).getByTestId('video-surface')).toBeInTheDocument();
  });
});
