import { QueryClient, dehydrate } from '@tanstack/react-query';

import { cameInIncognito, fromSaved, noteIncognito, toSaved } from './queryPersist';
import type { Saved } from './queryPersist';

describe('The saved query cache (M46.47)', () => {
  const answers = () => {
    const client = new QueryClient();
    client.setQueryData(['settings'], { theme: 'dark', accent: 'red' });
    client.setQueryData(['music', 'tags', 'for', ['a']], new Map([['a', [{ name: 'rock' }]]]));
    return dehydrate(client);
  };

  it('is kept compressed and reads back as it was', async () => {
    const state = answers();
    const saved = await toSaved(state, 'build-1', 5);
    expect(saved).toMatchObject({ build: 'build-1', savedAt: 5 });
    expect('packed' in saved && saved.packed).toBeInstanceOf(Uint8Array);
    expect(await fromSaved(saved)).toEqual(state);
  });

  it('still reads what earlier versions kept uncompressed', async () => {
    const state = answers();
    const before: Saved = { build: 'build-1', savedAt: 5, state };
    expect(await fromSaved(before)).toBe(state);
  });
});

describe("The saved query cache while incognito's on (M47)", () => {
  it('leaves out answers that came while it was on', () => {
    expect(cameInIncognito(5_000)).toBe(false);
    noteIncognito({ on: true, sinceMs: 10_000 });
    expect(cameInIncognito(9_999)).toBe(false);
    expect(cameInIncognito(10_000)).toBe(true);
    expect(cameInIncognito(Date.now())).toBe(true);
    // Turning on again doesn't start a new span.
    noteIncognito({ on: true, sinceMs: 20_000 });
    expect(cameInIncognito(15_000)).toBe(true);

    noteIncognito({ on: false, sinceMs: null });
    expect(cameInIncognito(15_000)).toBe(true);
    expect(cameInIncognito(Date.now() + 60_000)).toBe(false);
  });
});
