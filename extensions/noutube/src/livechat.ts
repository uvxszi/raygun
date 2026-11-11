// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/livechat.ts, and the live chat parts of content/player.ts.
//
// The mobile site shows no live chat. A button opens it: a live stream's chat is YouTube's own
// chat page in a frame, a past stream's is replayed as the video plays (./livechat-replay.ts).

import { findChatReplay, mountChatReplay } from './livechat-replay';
import { onNavigate, whenBody } from './page';
import {
  getPlayer,
  onVideoChange,
  playsMainVideo,
  readVideo,
  type Player,
  type VideoInfo,
} from './player';

let stopReplay: (() => void) | undefined;
let replayLookup: AbortController | undefined;

const SVG = 'http://www.w3.org/2000/svg';

/** An icon of the given path, made with DOM calls: the page takes no HTML from strings. */
function icon(size: number, path: string, stroke: boolean) {
  const svg = document.createElementNS(SVG, 'svg');
  svg.setAttribute('width', String(size));
  svg.setAttribute('height', String(size));
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('aria-hidden', 'true');
  if (stroke) {
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '2');
    svg.setAttribute('stroke-linecap', 'round');
  } else {
    svg.setAttribute('fill', 'currentColor');
  }
  const shape = document.createElementNS(SVG, 'path');
  shape.setAttribute('d', path);
  svg.append(shape);
  return svg;
}

const crossIcon = () => icon(24, 'm6 6 12 12M6 18 18 6', true);
const chatIcon = () =>
  icon(
    20,
    'M16 3v11H7.59L5 16.59V3h11m1-1H4v17l4-4h9V2zM8 18h8l4 4V6h-1v13.59L16.41 17H8v1z',
    false,
  );

function closeLiveChat() {
  stopReplay?.();
  stopReplay = undefined;
  document.querySelector('div#_nou_livechat')?.remove();
}

function showLiveChat(videoId: string, replayContinuation?: string) {
  closeLiveChat();
  const container = document.createElement('div');
  container.id = '_nou_livechat';
  const loading = document.createElement('div');
  loading.id = '_nou_livechat_loading';
  loading.textContent = 'Loading...';
  const close = document.createElement('button');
  close.className = '_nou_livechat_close';
  close.setAttribute('aria-label', 'Close chat');
  close.title = 'Close chat';
  close.append(crossIcon());
  close.onclick = closeLiveChat;
  container.append(loading, close);
  document.body.append(container);
  if (replayContinuation) {
    loading.remove();
    stopReplay = mountChatReplay(container, videoId, replayContinuation);
  } else {
    const frame = document.createElement('iframe');
    frame.src = `https://www.youtube.com/live_chat?v=${encodeURIComponent(videoId)}&embed_domain=${location.hostname}`;
    loading.after(frame);
  }
  container.classList.toggle(
    'right',
    window.innerWidth > 1000 && window.innerWidth > window.innerHeight,
  );
}

function hideLiveChat() {
  replayLookup?.abort();
  replayLookup = undefined;
  closeLiveChat();
  document.querySelector('button#_nou_livechat_btn')?.remove();
}

function showLiveChatButton(videoId: string, replayContinuation?: string) {
  let button = document.querySelector<HTMLButtonElement>('button#_nou_livechat_btn');
  const existed = Boolean(button);
  if (!button) {
    button = document.createElement('button');
    button.id = '_nou_livechat_btn';
  }
  button.replaceChildren(chatIcon(), replayContinuation ? 'Chat replay' : 'Live chat');
  button.onclick = () => showLiveChat(videoId, replayContinuation);
  if (!existed) document.body.append(button);
}

async function showLiveChatReplayButton(videoId: string) {
  replayLookup?.abort();
  const controller = new AbortController();
  replayLookup = controller;
  try {
    const continuation = await findChatReplay(videoId, controller.signal);
    if (continuation && !controller.signal.aborted) showLiveChatButton(videoId, continuation);
  } catch {
    // No replay button when YouTube turned the chat off or the lookup failed.
  }
}

/** The video the button is for; empty when there is none. */
let shownFor = '';

function update(video: VideoInfo | undefined, player: Player | undefined) {
  const id = video && playsMainVideo(player) ? video.id : '';
  if (id === shownFor) return;
  shownFor = id;
  hideLiveChat();
  if (!video || !id) return;
  if (video.live) showLiveChatButton(id);
  else if (video.liveContent) void showLiveChatReplayButton(id);
}

export function installLiveChat() {
  onVideoChange((video, player) => whenBody(() => update(video, player)));
  // Leaving the watch page leaves the chat with it, and coming to one finds its video's.
  onNavigate(() => {
    const player = getPlayer();
    whenBody(() => update(readVideo(player), player));
  });
}
