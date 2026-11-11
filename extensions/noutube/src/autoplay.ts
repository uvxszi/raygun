// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From how content/player.ts applies content/autoplay.ts.
//
// "Turn off autoplay" keeps YouTube's own Autoplay switch off, so no recommended video starts
// after the one watched; playlists and queues go on (./lib/autoplay.ts).

import { applyAutoplayPolicy, restoreKey } from './lib/autoplay';
import { coalesce, nouStorage, whenRoot } from './page';
import { onPlayerState } from './player';

export function installAutoplayPolicy(disabled: boolean) {
  // With the setting off there is only something to do while a choice waits to be given back.
  if (!disabled && nouStorage.getItem(restoreKey) === null) return;
  const apply = () => applyAutoplayPolicy(document, nouStorage, disabled);
  // The switch can come after the player, or be drawn again on another page; watching its
  // attributes also catches YouTube turning it back on.
  const later = coalesce(apply, 1000);
  whenRoot((root) => {
    apply();
    new MutationObserver(later).observe(root, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['aria-checked', 'aria-pressed'],
    });
  });
  onPlayerState(() => apply());
}
