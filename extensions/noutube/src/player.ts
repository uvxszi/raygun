// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/player.ts and content/player-progress.ts: finding YouTube's player as it comes
// and goes, and hearing what it plays, for the features that follow the video.

import { isObject, pick, pickString } from './lib/json';
import { isMobileSite, isMusic, whenRoot } from './page';

/** The calls of YouTube's player element (`#movie_player`) that the features use. */
export type Player = HTMLElement & {
  getPlayerState?: () => number;
  getCurrentTime?: () => number;
  getDuration?: () => number;
  getVideoData?: () => unknown;
  getPlayerResponse?: () => unknown;
  getWatchNextResponse?: () => unknown;
  playVideo?: () => void;
  pauseVideo?: () => void;
  seekTo?: (seconds: number, allowSeekAhead?: boolean) => void;
  getAvailableAudioTracks?: () => unknown;
  getAudioTrack?: () => unknown;
  setAudioTrack?: (track: unknown) => unknown;
};

/** The player's states, as `getPlayerState` and `onStateChange` say them. */
export const PlayerState = { ended: 0, playing: 1, paused: 2, buffering: 3 } as const;

/** What the player says of the video it has, from its player answer. */
export type VideoInfo = {
  id: string;
  title: string;
  author: string;
  lengthSeconds: number;
  /** A live stream, now. */
  live: boolean;
  /** Streamed live once: a replay may have its chat. */
  liveContent: boolean;
};

type VideoListener = (video: VideoInfo | undefined, player: Player | undefined) => void;
type TimeListener = (player: Player, video: HTMLVideoElement) => void;
type StateListener = (player: Player, state: number) => void;

const videoListeners: VideoListener[] = [];
const timeListeners: TimeListener[] = [];
const stateListeners: StateListener[] = [];

/** Calls `listener` when the player moves to another video (undefined: none, or the player went). */
export const onVideoChange = (listener: VideoListener) => void videoListeners.push(listener);

/** Calls `listener` on the video element's time updates, a few times a second while it plays. */
export const onTimeUpdate = (listener: TimeListener) => void timeListeners.push(listener);

/** Calls `listener` on each of the player's state changes. */
export const onPlayerState = (listener: StateListener) => void stateListeners.push(listener);

const report = (where: string, error: unknown) =>
  console.error(`[YouTube enhancements] ${where}:`, error);

/** The player element of the page, when there is one. */
export function getPlayer(): Player | undefined {
  const element = document.getElementById('movie_player');
  return element ? (element as Player) : undefined;
}

export function getVideoElement(player: Player | undefined): HTMLVideoElement | null {
  const video = player?.querySelector('video');
  return video instanceof HTMLVideoElement ? video : null;
}

/** The video the player has, from its player answer. */
export function readVideo(player: Player | undefined): VideoInfo | undefined {
  let response: unknown;
  try {
    response = player?.getPlayerResponse?.();
  } catch {
    return undefined;
  }
  const details = pick(response, 'videoDetails');
  const id = pickString(details, 'videoId');
  if (!id) return undefined;
  return {
    id,
    title: pickString(details, 'title') ?? '',
    author: pickString(details, 'author') ?? '',
    lengthSeconds: Number(pick(details, 'lengthSeconds')) || 0,
    live: Boolean(pick(response, 'playabilityStatus', 'liveStreamability')),
    liveContent: pick(details, 'isLiveContent') === true,
  };
}

/** The id of the video the player has, from the player itself (quicker than its answer). */
export function playerVideoId(player: Player | undefined): string {
  try {
    const data = player?.getVideoData?.();
    return (isObject(data) && typeof data.video_id === 'string' && data.video_id) || '';
  } catch {
    return '';
  }
}

/**
 * Whether the player plays the page's own video. The mobile site plays a feed's previews in the
 * same player, muted, and the desktop site has previews of its own; neither is followed. The
 * desktop site's mini player is: it goes on with the watched video on other pages.
 */
export function playsMainVideo(player: Player | undefined): boolean {
  if (!player?.isConnected) return false;
  if (isMusic || location.pathname === '/watch') return true;
  return !isMobileSite && !player.closest('ytd-video-preview, #inline-preview-player');
}

// YouTube's player takes its API events (onStateChange…) through addEventListener.
type ApiEvents = {
  addEventListener(type: string, listener: (value: unknown) => void): void;
  removeEventListener?(type: string, listener: (value: unknown) => void): void;
};

let player: Player | undefined;
let media: HTMLVideoElement | null = null;
let unbindMedia = () => {};
let unbindPlayer = () => {};
let videoId = '';

function emit<Listener extends (...args: never[]) => void>(
  listeners: Listener[],
  ...args: Parameters<Listener>
) {
  for (const listener of listeners) {
    try {
      listener(...args);
    } catch (error) {
      report('player', error);
    }
  }
}

function checkVideo() {
  const video = player ? readVideo(player) : undefined;
  const id = video?.id ?? '';
  if (id === videoId) return;
  videoId = id;
  emit(videoListeners, video, player);
}

// The player keeps its element while YouTube swaps the video element inside it.
function syncMedia() {
  const next = getVideoElement(player);
  if (next === media) return;
  unbindMedia();
  media = next;
  if (!next || !player) return;
  const owner = player;
  const onTime = () => {
    if (player === owner && media === next) emit(timeListeners, owner, next);
  };
  next.addEventListener('timeupdate', onTime);
  next.addEventListener('play', onTime);
  next.addEventListener('seeked', onTime);
  unbindMedia = () => {
    next.removeEventListener('timeupdate', onTime);
    next.removeEventListener('play', onTime);
    next.removeEventListener('seeked', onTime);
  };
}

function bind(element: Player) {
  if (element === player) {
    syncMedia();
    return;
  }
  unbindPlayer();
  player = element;
  const onStateChange = (value: unknown) => {
    if (player !== element) return;
    syncMedia();
    if (typeof value === 'number') emit(stateListeners, element, value);
    checkVideo();
  };
  const api = element as unknown as ApiEvents;
  api.addEventListener('onStateChange', onStateChange);
  unbindPlayer = () => {
    unbindMedia();
    unbindMedia = () => {};
    media = null;
    try {
      api.removeEventListener?.('onStateChange', onStateChange);
    } catch {
      // YouTube can take its player's API away before the element is let go.
    }
  };
  syncMedia();
  // The player can be found mid-playback (YouTube Music mounts it long after the page), when
  // the state change that would announce the video has already been.
  checkVideo();
}

function check() {
  const found = getPlayer();
  if (found) {
    bind(found);
  } else if (player && !player.isConnected) {
    unbindPlayer();
    unbindPlayer = () => {};
    player = undefined;
    checkVideo();
  }
}

let watching = false;

/** Starts following the page's player; the listeners above hear from it from then on. */
export function watchPlayer() {
  if (watching) return;
  watching = true;
  whenRoot((root) => {
    check();
    // YouTube swaps #movie_player out on its own, so the watch goes on for the page's life.
    new MutationObserver(check).observe(root, { childList: true, subtree: true });
  });
}
