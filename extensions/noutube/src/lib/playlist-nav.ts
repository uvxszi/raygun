// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/playlist-nav.ts.

import { isObject, pick, pickArray, pickString } from './json';

export type PlaylistDirection = 'previous' | 'next';

const getVideoId = (entry: unknown): string | undefined =>
  pickString(entry, 'playlistPanelVideoRenderer', 'videoId');

const getSelectedId = (playlist: unknown): string | undefined =>
  getVideoId(
    pickArray(playlist, 'contents').find((entry) =>
      Boolean(pick(entry, 'playlistPanelVideoRenderer', 'selected')),
    ),
  );

/** Whether the playlist was fetched for the playing video rather than for an earlier one. */
export const isPlaylistCurrent = (playlist: unknown, playingVideoId?: string) =>
  Boolean(playingVideoId) && getSelectedId(playlist) == playingVideoId;

/** The playlists of a watch next answer, the desktop site's layout included. */
function getPlaylists(response: unknown): unknown[] {
  return ['singleColumnWatchNextResults', 'twoColumnWatchNextResults']
    .map((layout) => pick(response, 'contents', layout, 'playlist', 'playlist'))
    .filter((playlist) => Array.isArray(pick(playlist, 'contents')));
}

/** The playlist of the player's watch next answer, when that is about the playing video. */
export function getPlayingPlaylist(response: unknown, playingVideoId?: string): unknown {
  return getPlaylists(response).find((playlist) => isPlaylistCurrent(playlist, playingVideoId));
}

/**
 * The playlist `listId` out of answers that may be about an earlier video.
 *
 * YouTube sometimes moves on to the next video without fetching its watch next answer, mostly
 * while the page is out of sight, and every answer the page holds is then about an earlier
 * video. Such a playlist still lists the playing one. It lingers after the playlist was left
 * too, so it only counts while the page's address still names it.
 */
export function getHeldPlaylist(
  responses: unknown[],
  playingVideoId?: string,
  listId?: string | null,
): unknown {
  if (!playingVideoId || !listId) return undefined;
  const playlists = responses
    .flatMap(getPlaylists)
    .filter((playlist) => pick(playlist, 'playlistId') == listId);
  return (
    playlists.find((playlist) => isPlaylistCurrent(playlist, playingVideoId)) ||
    playlists.find((playlist) =>
      pickArray(playlist, 'contents').some((entry) => getVideoId(entry) == playingVideoId),
    )
  );
}

function getEndpointUrl(endpoint: unknown, playlistId?: string): string | undefined {
  const url = pickString(endpoint, 'commandMetadata', 'webCommandMetadata', 'url');
  if (url?.includes('v=')) return url;
  const videoId = pickString(endpoint, 'watchEndpoint', 'videoId');
  if (!videoId) return undefined;
  const list = pickString(endpoint, 'watchEndpoint', 'playlistId') || playlistId;
  return `/watch?v=${videoId}${list ? `&list=${list}` : ''}`;
}

/**
 * The address of the video before or after the playing one in a playlist, read from the
 * playlist of m.youtube.com's watch next answer. The mobile player does not know the playlist
 * it is in, so its own next and previous leave it.
 *
 * Undefined when `playingVideoId` is not in the playlist, or when the playlist has nothing in
 * that direction.
 */
export function getPlaylistNeighborUrl(
  data: unknown,
  direction: PlaylistDirection,
  playingVideoId?: string,
): string | undefined {
  if (!isObject(data)) return undefined;
  const contents = pickArray(data, 'contents');
  const playlistId = pickString(data, 'playlistId');
  const selectedId = getSelectedId(data);
  const playingId = playingVideoId || selectedId;
  const current = Boolean(selectedId) && selectedId == playingId;
  // A stale playlist has its selection on an earlier video.
  const playingIndex = contents.findIndex((entry) =>
    current
      ? Boolean(pick(entry, 'playlistPanelVideoRenderer', 'selected'))
      : getVideoId(entry) == playingId,
  );
  if (!playingId || playingIndex < 0) return undefined;

  // YouTube's own skip buttons, which also follow shuffle and loop. They skip from the selected
  // video, so they only help while that is the playing one.
  const button = direction == 'next' ? data.nextButtonVideo : data.previousButtonVideo;
  const command = pick(button, 'buttonRenderer', 'command');
  if (current && pickString(command, 'watchEndpoint', 'videoId') != selectedId) {
    const url = getEndpointUrl(command, playlistId);
    if (url) return url;
  }

  const neighbor = pick(
    contents[playingIndex + (direction == 'next' ? 1 : -1)],
    'playlistPanelVideoRenderer',
  );
  const neighborId = pickString(neighbor, 'videoId');
  if (!neighborId) return undefined;
  return (
    getEndpointUrl(pick(neighbor, 'navigationEndpoint'), playlistId) ||
    getEndpointUrl({ watchEndpoint: { videoId: neighborId } }, playlistId)
  );
}
