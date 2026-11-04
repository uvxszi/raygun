import { cameInIncognito, noteIncognito } from './queryPersist';

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
