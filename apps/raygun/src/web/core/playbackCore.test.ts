import type { QueueItemInput } from '@raygun/bindings';

import { PlaybackCore } from './playbackCore';
import type { Effect } from './playbackCore';

const songs = (count: number): QueueItemInput[] =>
  Array.from({ length: count }, (_, index) => ({
    title: `Song ${index}`,
    artist: 'Artist',
    album: null,
    durationMs: 200_000,
    artworkUrl: null,
    mediaKind: 'audio',
    source: { kind: 'url', url: `https://example.com/${index}.mp3`, headers: {} },
  }));

const coreWith = () => {
  let next = 0;
  return new PlaybackCore(42n, () => `item-${(next += 1)}`);
};

const loaded = (effects: Effect[]) =>
  effects.flatMap((effect) => (effect.type === 'load' ? [effect.item.title] : []));

const current = (core: PlaybackCore) => core.currentItem()?.title;

describe('the playback core (ported from raygun-playback)', () => {
  it('replaces the queue and plays from the start item', () => {
    const core = coreWith();
    const effects = core.replace(songs(3), 1, false);
    expect(loaded(effects)).toEqual(['Song 1']);
    expect(core.state().status).toBe('loading');
    expect(loaded(core.next())).toEqual(['Song 2']);
    expect(core.next()).toEqual([]);
  });

  it('wraps only with repeat all, and restarts the song when far in', () => {
    const core = coreWith();
    core.replace(songs(2), 1, false);
    core.setRepeat('all');
    expect(loaded(core.next())).toEqual(['Song 0']);
    core.report({ loadId: core.currentLoadId, event: { type: 'playing' } });
    core.report({
      loadId: core.currentLoadId,
      event: { type: 'progress', positionMs: 10_000, durationMs: 200_000 },
    });
    expect(core.previous()).toContainEqual({
      type: 'engine',
      command: { type: 'seek', positionMs: 0 },
    });
    expect(loaded(core.previous())).toEqual(['Song 1']);
  });

  it('shuffles with the playing song first, and back to queue order', () => {
    const core = coreWith();
    core.replace(songs(6), 2, false);
    core.setShuffle(true);
    expect(current(core)).toBe('Song 2');
    expect(core.queue().order[0]).toBe(2);
    expect([...core.queue().order].sort()).toEqual([0, 1, 2, 3, 4, 5]);
    core.setShuffle(false);
    expect(core.queue().order).toEqual([0, 1, 2, 3, 4, 5]);
    expect(current(core)).toBe('Song 2');
  });

  it('moves on when the playing song is removed, and stops at the end', () => {
    const core = coreWith();
    core.replace(songs(2), 0, false);
    const first = core.currentItem()?.id ?? '';
    expect(loaded(core.remove(first))).toEqual(['Song 1']);
    const last = core.currentItem()?.id ?? '';
    core.remove(last);
    expect(core.state().status).toBe('idle');
    expect(core.queue().items).toEqual([]);
  });

  it('skips songs that fail, and stops after three in a row', () => {
    const core = coreWith();
    core.replace(songs(5), 0, false);
    const fail = () =>
      core.report({ loadId: core.currentLoadId, event: { type: 'error', message: 'gone' } });
    expect(loaded(fail())).toEqual(['Song 1']);
    expect(loaded(fail())).toEqual(['Song 2']);
    expect(fail()).toContainEqual({ type: 'engine', command: { type: 'stop' } });
    expect(core.state()).toMatchObject({ status: 'error', error: 'gone' });
  });

  it('keeps a pause even when the engine starts late', () => {
    const core = coreWith();
    core.replace(songs(1), 0, false);
    core.pause();
    const effects = core.report({ loadId: core.currentLoadId, event: { type: 'playing' } });
    expect(effects).toContainEqual({ type: 'engine', command: { type: 'pause' } });
    expect(core.state().status).toBe('paused');
  });

  it('clears what comes after the playing song', () => {
    const core = coreWith();
    core.replace(songs(4), 1, false);
    core.clearUpcoming();
    expect(core.queue().items.map((item) => item.title)).toEqual(['Song 0', 'Song 1']);
    expect(current(core)).toBe('Song 1');
  });

  it('ignores reports about older loads', () => {
    const core = coreWith();
    core.replace(songs(2), 0, false);
    const old = core.currentLoadId;
    core.next();
    expect(core.report({ loadId: old, event: { type: 'ended' } })).toEqual([]);
    expect(current(core)).toBe('Song 1');
  });
});
