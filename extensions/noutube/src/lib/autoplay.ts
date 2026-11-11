// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/autoplay.ts.
//
// YouTube's own Autoplay switch is used: the mobile site's countdown doesn't follow the
// player's setAutonavState. The listener's choice is saved, so it can be given back once the
// setting is turned off, even after a reload.

export const restoreKey = 'nou:autoplay-restore';
const toggleSelector = [
  '.ytwAutonavToggleButtonHost',
  '.ytp-autonav-toggle-button',
  'ytm-autonav-toggle-button-renderer button',
  'ytm-autonav-toggle-button-renderer [role="switch"]',
].join(', ');

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

let clicking = false;
const lastClicks = new WeakMap<object, number>();

export function applyAutoplayPolicy(
  root: Pick<ParentNode, 'querySelectorAll'>,
  storage: StorageLike,
  disabled: boolean,
) {
  if (clicking) return;
  if (!disabled && storage.getItem(restoreKey) === null) return;
  for (const toggle of root.querySelectorAll<HTMLElement>(toggleSelector)) {
    const value = toggle.getAttribute('aria-checked') ?? toggle.getAttribute('aria-pressed');
    if (value !== 'true' && value !== 'false') continue;
    const enabled = value === 'true';
    const original = storage.getItem(restoreKey);
    if (disabled && original === null) storage.setItem(restoreKey, value);
    const desired = disabled
      ? false
      : original === 'true'
        ? true
        : original === 'false'
          ? false
          : undefined;
    if (desired === undefined) return;
    if (enabled !== desired) {
      const now = Date.now();
      const last = lastClicks.get(toggle);
      if (last !== undefined && now - last < 1000) return;
      lastClicks.set(toggle, now);
      clicking = true;
      try {
        toggle.click();
      } finally {
        clicking = false;
      }
      // The switch updates later. The marker stays until its state says the click took, also
      // when a new control takes its place.
      return;
    }
    if (!disabled) storage.removeItem(restoreKey);
    return;
  }
}
