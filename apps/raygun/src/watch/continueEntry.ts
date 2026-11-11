import type { ContinueWatching } from '@raygun/bindings';

import { STREMIO_SOURCE, videoTitleLink } from './useVideoLibrary';
import { videoLinkPage } from './useVideoLink';

/** Where an entry opens: its title's page, or a link played on its own's (`/watch/link`). */
export const entryPage = (item: ContinueWatching) =>
  item.link
    ? videoLinkPage(item.key)
    : item.sourceId && item.remoteId
      ? videoTitleLink(item.sourceId, item.remoteId)
      : null;

/**
 * A link played on its own that can be downloaded: a web address or a torrent, not a file on
 * this device or a DASH stream.
 */
export const linkDownloads = (item: ContinueWatching) =>
  Boolean(item.link) &&
  /^url:(https?:|magnet:)/i.test(item.key) &&
  !/\.mpd([?#]|$)/i.test(item.key);

/** Videos from Local Files folders: on this device, with no other source to choose. */
export const LOCAL_VIDEO = 'local.video';

/** A video source's stored title in an entry's player group (`title:<id>`). */
export const storedTitle = (item: ContinueWatching) =>
  item.group?.startsWith('title:') ? item.group.slice('title:'.length) : null;

/** A Stremio title's type and id, from its `<type>:<id>`. */
export const stremioParts = (remoteId: string) => {
  const split = remoteId.indexOf(':');
  return { kind: remoteId.slice(0, split), id: remoteId.slice(split + 1) };
};

/** Started: an up next episode (Continue watching moved on to it) is at 1 ms. */
export const isStarted = (item: ContinueWatching) => !item.watched && (item.positionMs ?? 0) > 1;

/**
 * Whether an entry's source can be chosen: a Stremio title's streams, or the servers of a video
 * source's stored episode (not a file on this device).
 */
export const canChooseSource = (item: ContinueWatching) =>
  item.sourceId === STREMIO_SOURCE
    ? Boolean(item.remoteId && item.episodeId)
    : Boolean(
        item.sourceId &&
        item.sourceId !== LOCAL_VIDEO &&
        storedTitle(item) &&
        item.unitId &&
        item.episodeId,
      );
