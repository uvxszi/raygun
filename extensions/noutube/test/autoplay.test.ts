// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/autoplay.test.ts.

import { describe, expect, it, vi } from 'vitest';

import { applyAutoplayPolicy } from '../src/lib/autoplay';

function page(initial = true, attribute = 'aria-pressed') {
  let enabled = initial;
  let mounted = true;
  const calls: boolean[] = [];
  const values = new Map<string, string>();
  const storage = {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
    removeItem: (key: string) => {
      values.delete(key);
    },
  };
  const toggle = {
    getAttribute: (key: string) => (key === attribute ? String(enabled) : null),
    click: () => {
      enabled = !enabled;
      calls.push(enabled);
    },
  };
  const root = { querySelectorAll: () => (mounted ? [toggle] : []) } as unknown as Document;
  return {
    root,
    storage,
    calls,
    values,
    mount: (value: boolean) => {
      mounted = value;
    },
  };
}

describe('recommendation autoplay policy', () => {
  for (const attribute of ['aria-pressed', 'aria-checked']) {
    it(`uses the native ${attribute} toggle and restores the original preference`, () => {
      const clock = vi.spyOn(Date, 'now').mockReturnValue(10000);
      try {
        const current = page(true, attribute);
        applyAutoplayPolicy(current.root, current.storage, false);
        expect(current.calls).toEqual([]);
        applyAutoplayPolicy(current.root, current.storage, true);
        applyAutoplayPolicy(current.root, current.storage, true);
        expect(current.calls).toEqual([false]);
        clock.mockReturnValue(12000);
        applyAutoplayPolicy(current.root, current.storage, false);
        applyAutoplayPolicy(current.root, current.storage, false);
        expect(current.calls).toEqual([false, true]);
        expect(current.values.size).toBe(0);
      } finally {
        clock.mockRestore();
      }
    });
  }

  it('does not enable autoplay that was already off', () => {
    const current = page(false);
    applyAutoplayPolicy(current.root, current.storage, true);
    applyAutoplayPolicy(current.root, current.storage, false);
    expect(current.calls).toEqual([]);
    expect(current.values.size).toBe(0);
  });

  it('restores after page/player replacement using the persisted marker', () => {
    const current = page();
    applyAutoplayPolicy(current.root, current.storage, true);
    const replacement = page(false);
    applyAutoplayPolicy(replacement.root, current.storage, false);
    expect(replacement.calls).toEqual([true]);
  });

  it('waits for controls to mount without losing the original choice', () => {
    const current = page();
    current.mount(false);
    applyAutoplayPolicy(current.root, current.storage, true);
    expect(current.values.size).toBe(0);
    current.mount(true);
    applyAutoplayPolicy(current.root, current.storage, true);
    expect(current.calls).toEqual([false]);
  });

  it('does not repeatedly click a control that ignores clicks', () => {
    let clicks = 0;
    const toggle = {
      getAttribute: () => 'true',
      click: () => {
        clicks++;
      },
    };
    const root = { querySelectorAll: () => [toggle] } as unknown as Document;
    const current = page();
    applyAutoplayPolicy(root, current.storage, true);
    applyAutoplayPolicy(root, current.storage, true);
    expect(clicks).toBe(1);
  });
});
