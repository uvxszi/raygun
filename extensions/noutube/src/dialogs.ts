// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/dialogs.ts: YouTube Music's promotion banners (the "mealbar") are dismissed as
// they open.

import { whenRoot } from './page';

let handled = false;

function watchDialog() {
  if (handled) return;
  const target = document.querySelector('tp-yt-paper-dialog');
  if (!target) return;
  handled = true;
  new MutationObserver(() => {
    const dismiss = target.querySelector<HTMLElement>(
      'ytmusic-mealbar-promo-renderer .dismiss-button',
    );
    dismiss?.click();
  }).observe(target, { attributes: true });
}

/** For YouTube Music. */
export function installPromoDismissal() {
  whenRoot((root) => {
    const observer = new MutationObserver(() => {
      watchDialog();
      if (handled) observer.disconnect();
    });
    observer.observe(root, { childList: true, subtree: true });
    watchDialog();
    if (handled) observer.disconnect();
  });
}
