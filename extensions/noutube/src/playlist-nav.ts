// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From skipInPlaylist in content/noutube.ts and navigateWatch in content/split-view.ts.
//
// The mobile site keeps a playlist in the page rather than in the player, so the player's own
// next and previous (which the media notification's buttons reach) leave the playlist for a
// recommendation. While a playlist plays, those presses go to its next or previous video.

import {
  getHeldPlaylist,
  getPlayingPlaylist,
  getPlaylistNeighborUrl,
  isPlaylistCurrent,
  type PlaylistDirection,
} from './lib/playlist-nav';
import { hookMediaAction } from './media-session';
import { getPlayer, playerVideoId } from './player';

/** Opens a watch page through the page's own links, so it doesn't load the site again. */
function navigateWatch(url: string) {
  try {
    const target = new URL(url, location.href);
    // The router takes only links of its own origin.
    target.host = location.host;
    const anchor = document.createElement('a');
    anchor.href = target.href;
    anchor.style.display = 'none';
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
  } catch {
    location.href = url;
  }
}

function skipInPlaylist(direction: PlaylistDirection): boolean {
  const player = getPlayer();
  if (!player) return false;
  // The playlist panel exists only while it is open, so what the page gave the player is read.
  const videoId = playerVideoId(player) || undefined;
  let response: unknown;
  try {
    response = player.getWatchNextResponse?.();
  } catch {
    response = undefined;
  }
  // Right after a page load the player has no answer yet, while the page's own is there.
  const playlist =
    getPlayingPlaylist(response, videoId) ||
    getHeldPlaylist(
      [response, window.ytInitialData],
      videoId,
      new URLSearchParams(location.search).get('list'),
    );
  if (!playlist) return false;
  const url = getPlaylistNeighborUrl(playlist, direction, videoId);
  // At either end the press is the playlist's too: the player's own would start a
  // recommendation or jump back into unrelated history.
  if (!url) return true;
  if (isPlaylistCurrent(playlist, videoId)) {
    navigateWatch(url);
  } else {
    // The page stopped fetching the playlist, and going on inside it would leave it so; loading
    // the video afresh gets it going again.
    location.assign(url);
  }
  return true;
}

/** For the mobile site; needs the Media Session wrapped at document start (./media-session.ts). */
export function installPlaylistControls() {
  hookMediaAction('nexttrack', () => skipInPlaylist('next'));
  hookMediaAction('previoustrack', () => skipInPlaylist('previous'));
}
