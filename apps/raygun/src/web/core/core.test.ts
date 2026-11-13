import { commands, events } from '@raygun/bindings';
import type { EngineCommand, PlaybackState, TrackInput } from '@raygun/bindings';

import { NOT_ON_THE_WEB } from './fallback';
import { setRemoteHandler } from './index';
import { openCore } from '../test/webCore';
import type { TestCore } from '../test/webCore';

const track = (title: string, artist: string, extra: Partial<TrackInput> = {}): TrackInput => ({
  title,
  artist,
  album: null,
  durationMs: 200_000,
  artworkUrl: null,
  source: null,
  ...extra,
});

const ok = <T>(answer: { status: 'ok'; data: T } | { status: 'error'; error: unknown }): T => {
  if (answer.status !== 'ok') throw new Error(`expected ok, got ${String(answer.error)}`);
  return answer.data;
};

let open: TestCore | null = null;
const start = async (...args: Parameters<typeof openCore>) => {
  open?.ipc.uninstall();
  open = await openCore(...args);
  return open;
};

afterEach(() => {
  open?.ipc.uninstall();
  open = null;
});

describe('the web core behind the bindings', () => {
  it('says it is Raygun on the web', async () => {
    await start();
    const info = await commands.appInfo();
    expect(info).toMatchObject({ name: 'Raygun', version: '9.9.9', platform: 'web' });
    expect((await commands.deviceState()).foreground).toBe(true);
  });

  it('keeps settings across a reload, over Rust’s defaults', async () => {
    const first = await start();
    const defaults = await commands.settingsDefaults();
    expect(defaults.windowFrame).toBe('system');
    const changed = ok(await commands.settingsUpdate({ key: 'accent', value: 'teal' }));
    expect(changed.accent).toBe('teal');
    const tables = await first.saved();
    const setting = tables.setting?.find(([key]) => key === 'accent')?.[1] as {
      value: string;
      updated_at_hlc: bigint;
    };
    expect(setting.value).toBe('"teal"');
    expect(typeof setting.updated_at_hlc).toBe('bigint');

    await start(tables);
    const reloaded = ok(await commands.settingsGet());
    expect(reloaded.accent).toBe('teal');
    expect(reloaded.theme).toBe(defaults.theme);
  });

  it('turns the "No 18+" pill on with 18+ content, as Rust does', async () => {
    await start();
    const settings = ok(await commands.settingsUpdate({ key: 'showNsfw', value: true }));
    expect(settings.searchFilter.noAdult).toBe(true);
  });

  it('keeps favorites and playlists', async () => {
    const first = await start();
    ok(await commands.favoriteSet(track('Song A', 'Artist'), true));
    ok(await commands.favoriteSet(track('Song B', 'Artist', { durationMs: 1234.6 }), true));
    // Favoriting again, under a decorated title, finds the same song.
    ok(await commands.favoriteSet(track('Song A - Remastered 2011', 'Artist'), true));
    const favorites = ok(await commands.favoritesList());
    expect(favorites.map((favorite) => favorite.title)).toEqual(['Song B', 'Song A']);
    expect(favorites[0]?.durationMs).toBe(1235);

    const playlist = ok(await commands.playlistCreate('  Road trip  '));
    expect(playlist.name).toBe('Road trip');
    const added = ok(
      await commands.playlistAdd(
        playlist.id,
        [track('Song A', 'Artist'), track('Song C', 'Other')],
        false,
      ),
    );
    expect(added).toEqual({ added: 2, already: 0 });
    const again = ok(await commands.playlistAdd(playlist.id, [track('Song A', 'Artist')], false));
    expect(again).toEqual({ added: 0, already: 1 });
    const saves = ok(await commands.collectionsSaves(track('Song A', 'Artist')));
    expect(saves).toEqual({ favorite: true, playlistIds: [playlist.id] });

    const detail = ok(await commands.playlistGet(playlist.id));
    const [first_, second] = detail.items;
    ok(await commands.playlistMoveItem(second!.id, first_!.id));
    const moved = ok(await commands.playlistGet(playlist.id));
    expect(moved.items.map((item) => item.track.title)).toEqual(['Song C', 'Song A']);

    await start(await first.saved());
    expect(ok(await commands.playlistsList()).map((list) => [list.name, list.itemCount])).toEqual([
      ['Road trip', 2],
    ]);
    ok(await commands.favoriteSet(track('Song A', 'Artist'), false));
    expect(ok(await commands.favoritesList()).map((favorite) => favorite.title)).toEqual([
      'Song B',
    ]);
    expect((await commands.playlistCreate('   ')).status).toBe('error');
  });

  it('answers commands it has no implementation of', async () => {
    await start();
    expect(ok(await commands.libraryStatus())).toEqual({
      roots: [],
      trackCount: 0,
      scanning: false,
    });
    const refused = await commands.podcastSetSubscribed('feed', true);
    expect(refused).toEqual({ status: 'error', error: NOT_ON_THE_WEB });
    await expect(commands.ttsSetAvailable(true)).resolves.toBeNull();
  });

  it("lets a remote answer first, with the page's own answer to fall back to", async () => {
    await start();
    const asked: string[] = [];
    setRemoteHandler((command, _args, local) => {
      asked.push(command);
      if (command === 'podcast_set_subscribed') return Promise.resolve(null);
      return command === 'library_status' ? local() : null;
    });
    try {
      expect(await commands.podcastSetSubscribed('feed', true)).toEqual({
        status: 'ok',
        data: null,
      });
      expect(ok(await commands.libraryStatus()).trackCount).toBe(0);
      ok(await commands.playlistsList());
      expect(asked).toEqual(['podcast_set_subscribed', 'library_status', 'playlists_list']);
    } finally {
      setRemoteHandler(null);
    }
  });

  it('plays through the page’s engine and counts the play in history', async () => {
    await start();
    const loads: EngineCommand[] = [];
    const states: PlaybackState[] = [];
    const stopEngine = await events.engineCommandEvent.listen(({ payload }) => loads.push(payload));
    const stopState = await events.playbackStateEvent.listen(({ payload }) => states.push(payload));
    const song = track('Streamed', 'Artist', {
      source: { kind: 'url', url: 'https://example.com/song.mp3', headers: {} },
      durationMs: 100_000,
    });
    ok(await commands.favoriteSet(song, true));
    ok(
      await commands.collectionPlay({
        collection: { kind: 'favorites' },
        startIndex: 0,
        shuffle: false,
      }),
    );
    await vi.waitFor(() => expect(loads.some((command) => command.type === 'load')).toBe(true));
    const load = loads.find((command) => command.type === 'load');
    if (load?.type !== 'load') throw new Error('no load');
    expect(load.url).toBe('https://example.com/song.mp3');
    expect(load.autoplay).toBe(true);

    await commands.playbackEngineReport({ loadId: load.loadId, event: { type: 'playing' } });
    for (let position = 0; position <= 52_000; position += 1_000) {
      await commands.playbackEngineReport({
        loadId: load.loadId,
        event: { type: 'progress', positionMs: position, durationMs: 100_000 },
      });
    }
    await vi.waitFor(() => expect(states.at(-1)?.status).toBe('playing'));
    const history = ok(await commands.historyRecent(10));
    expect(history.map((entry) => entry.track.title)).toEqual(['Streamed']);
    stopEngine();
    stopState();
  });

  it('says a song with nothing to play from can’t play here', async () => {
    await start();
    const states: PlaybackState[] = [];
    const stop = await events.playbackStateEvent.listen(({ payload }) => states.push(payload));
    await commands.playbackQueueTracks([track('Lookup only', 'Artist')], false);
    await commands.playbackControl({ type: 'play' });
    await vi.waitFor(() => expect(states.at(-1)?.status).toBe('error'));
    expect(states.at(-1)?.error).toMatch(/No source in this browser/);
    stop();
  });
});
