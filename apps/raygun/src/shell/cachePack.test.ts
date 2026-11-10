import type { DehydratedState } from '@tanstack/react-query';

import { canCompress, gunzipText, gzipText, packQueries, unpackQueries } from './cachePack';

type Query = DehydratedState['queries'][number];

/** A saved answer as react-query's `dehydrate` writes it. */
const query = (hash: string, data: unknown): Query =>
  ({
    queryHash: hash,
    queryKey: JSON.parse(hash) as unknown[],
    dehydratedAt: 1_000,
    state: {
      data,
      dataUpdateCount: 1,
      dataUpdatedAt: 900,
      error: null,
      errorUpdateCount: 0,
      errorUpdatedAt: 0,
      fetchFailureCount: 0,
      fetchFailureReason: null,
      fetchMeta: null,
      isInvalidated: false,
      status: 'success',
      fetchStatus: 'idle',
    },
  }) as Query;

const stateOf = (...queries: Query[]): DehydratedState => ({ mutations: [], queries });

const roundTrip = (state: DehydratedState) => {
  const { json, tagged } = packQueries(state);
  return unpackQueries(json, tagged);
};

describe('The saved query cache packed as JSON', () => {
  it('reads back plain answers exactly, walking none of them', () => {
    const state = stateOf(
      query('["settings"]', { theme: 'dark', zoom: 100, pins: [{ to: '/', name: 'Home' }] }),
      query('["music","charts","us"]', [{ title: 'Song', artist: null, durationMs: 180_000 }]),
    );
    const { json, tagged } = packQueries(state);
    expect(tagged).toEqual([]);
    expect(unpackQueries(json, tagged)).toEqual(state);
  });

  it('keeps Maps, Sets, Dates, odd numbers and holes in lists as they were', () => {
    const data = {
      tags: new Map([
        ['song-a', [{ name: 'rock', weight: 1 }]],
        ['song-b', []],
      ]),
      seen: new Set(['a', 'b']),
      at: new Date(1_700_000_000_000),
      ratio: Number.NaN,
      cap: Number.POSITIVE_INFINITY,
      list: [1, undefined, 3],
      nested: [{ more: new Map([[1, new Set([2])]]) }],
    };
    const { json, tagged } = packQueries(stateOf(query('["music","tags","for"]', data)));
    expect(tagged).toEqual(['["music","tags","for"]']);

    const [read] = unpackQueries(json, tagged).queries;
    const back = read?.state.data as typeof data;
    expect(back.tags).toBeInstanceOf(Map);
    expect(back.tags.get('song-a')).toEqual([{ name: 'rock', weight: 1 }]);
    expect(back.seen).toEqual(new Set(['a', 'b']));
    expect(back.at).toEqual(new Date(1_700_000_000_000));
    expect(back.ratio).toBeNaN();
    expect(back.cap).toBe(Number.POSITIVE_INFINITY);
    expect(back.list).toEqual([1, undefined, 3]);
    expect(back.nested[0]?.more.get(1)).toEqual(new Set([2]));
  });

  it('writes an unchanged answer out once, while the rest of its query follows', () => {
    let reads = 0;
    const answer = {
      get titles() {
        reads += 1;
        return Array.from({ length: 3 }, (_, index) => ({ title: `Song ${index}`, at: index }));
      },
    };
    const first = query('["collections","favorites"]', answer);
    expect(roundTrip(stateOf(first)).queries[0]?.state.data).toEqual({
      titles: answer.titles,
    });
    reads = 0;

    // Saved again with a newer fetch: the same answer object, so its JSON is reused.
    const later = { ...first, state: { ...first.state, dataUpdateCount: 2, dataUpdatedAt: 950 } };
    const [back] = roundTrip(stateOf(later)).queries;
    expect(reads).toBe(0);
    expect(back?.state.dataUpdateCount).toBe(2);
    expect(back?.state.dataUpdatedAt).toBe(950);
    expect(back?.state.data).toEqual({ titles: answer.titles });

    // A new answer is written out again.
    const changed = query('["collections","favorites"]', { titles: [] });
    expect(roundTrip(stateOf(changed)).queries[0]?.state.data).toEqual({ titles: [] });
  });

  it('leaves out answers JSON would change, and keeps the rest', () => {
    class Track {
      title = 'Song';
    }
    const kept = query('["kept"]', { title: 'Song' });
    const state = stateOf(
      query('["instance"]', new Track()),
      query('["bytes"]', { picture: new Uint8Array([1, 2]) }),
      query('["big"]', { count: BigInt(1) }),
      query('["clash"]', { '~raygun': 'map' }),
      kept,
    );
    expect(roundTrip(state).queries).toEqual([kept]);
  });

  it('compresses text and reads it back', async () => {
    expect(canCompress()).toBe(true);
    const text = JSON.stringify(
      Array.from({ length: 300 }, (_, index) => ({ title: `Song ${index}`, artist: 'Artist' })),
    );
    const packed = await gzipText(text);
    expect(packed.byteLength).toBeLessThan(text.length / 5);
    expect(await gunzipText(packed)).toBe(text);
    await expect(gunzipText(new TextEncoder().encode(text))).rejects.toThrow();
  });
});
