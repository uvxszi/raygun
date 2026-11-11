// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/author-names.ts.
//
// The mobile site sometimes shows the channel's name percent-encoded (`%E4%BD%A0%E5%A5%BD`
// instead of the name). The player answer carries the name itself, which is put in its place.
// The `%XX` test means this does nothing once YouTube stops showing the encoded form.

import { pickString } from './lib/json';
import { coalesce, whenRoot } from './page';
import { getPlayer } from './player';

const fixEncodedAuthorNames = () => {
  if (location.pathname !== '/watch') return;

  let author: string | undefined;
  try {
    author = pickString(getPlayer()?.getPlayerResponse?.(), 'videoDetails', 'author');
  } catch {
    return;
  }
  if (!author) return;

  document
    .querySelectorAll(
      'ytm-slim-video-information-renderer .slim-video-information-channel-name .ytAttributedStringHost, ' +
        'ytm-slim-owner-renderer .ytAttributedStringHost',
    )
    .forEach((label) => {
      if (!/%[0-9a-f]{2}/i.test(label.textContent || '')) return;

      const parts = label.querySelectorAll(':scope > span');
      const last = parts[parts.length - 1];
      const name = parts.length > 1 && parts[0]?.textContent === '@' && last ? last : label;
      const text = name === label ? '@' + author : author;
      if (name.textContent !== text) name.textContent = text;
    });
};

/** For the mobile site, whose renderers these are. */
export function installEncodedAuthorNameFix() {
  const fix = coalesce(fixEncodedAuthorNames, 250);
  whenRoot((root) => {
    new MutationObserver(fix).observe(root, { childList: true, subtree: true });
    fixEncodedAuthorNames();
  });
}
