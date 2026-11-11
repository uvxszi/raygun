// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/livechat-replay.ts: a past stream's chat, replayed beside the video as it plays.

import { isObject, pick, pickArray, pickString } from './lib/json';
import {
  ChatReplayBuffer,
  chatReplayImageUrl,
  chatReplayPollDelay,
  getChatReplay,
  shouldPrefetchChatReplay,
  type ReplayMessage,
} from './lib/live-chat-replay';
import { getYouTubeAuthorization } from './lib/youtube-auth';
import { pageFetch, ytcfgValue } from './page';
import { PlayerState, getPlayer, getVideoElement, readVideo } from './player';

async function request(path: string, data: object, signal: AbortSignal): Promise<unknown> {
  const context = ytcfgValue('INNERTUBE_CONTEXT');
  const client = pick(context, 'client');
  if (!isObject(context) || !isObject(client)) throw new Error('YouTube context is not ready');
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  // Secure SID cookies can outlive signing out; a signed-out page asks without them.
  const authorization =
    ytcfgValue('LOGGED_IN') === false
      ? undefined
      : await getYouTubeAuthorization(location.origin, document.cookie);
  if (authorization) {
    headers.Authorization = authorization;
    headers['X-Goog-AuthUser'] = String(ytcfgValue('SESSION_INDEX') ?? 0);
    headers['X-Origin'] = location.origin;
    const pageId = ytcfgValue('DELEGATED_SESSION_ID');
    if (typeof pageId === 'string' && pageId) headers['X-Goog-PageId'] = pageId;
  }
  if (typeof client.visitorData === 'string' && client.visitorData) {
    headers['X-Goog-Visitor-Id'] = client.visitorData;
  }
  const response = await pageFetch(`${location.origin}/youtubei/v1/${path}?prettyPrint=false`, {
    method: 'POST',
    headers,
    signal,
    body: JSON.stringify({
      context: {
        ...context,
        client: {
          ...client,
          clientName: 'WEB',
          platform: 'DESKTOP',
          clientFormFactor: 'UNKNOWN_FORM_FACTOR',
        },
      },
      ...data,
    }),
  });
  if (!response.ok) throw new Error(`Chat request failed: ${response.status}`);
  return response.json();
}

export async function findChatReplay(videoId: string, signal: AbortSignal) {
  // The mobile site's answers leave the chat out. The desktop site's are asked for on this
  // origin, so the page's own cookies still apply without switching the page to desktop.
  return getChatReplay(await request('next', { videoId }, signal));
}

function appendText(element: HTMLElement, text: unknown) {
  const simpleText = pickString(text, 'simpleText');
  if (simpleText) element.append(document.createTextNode(simpleText));
  for (const run of pickArray(text, 'runs')) {
    const runText = pickString(run, 'text');
    if (runText) element.append(document.createTextNode(runText));
    const emoji = pick(run, 'emoji');
    if (!isObject(emoji)) continue;
    const image = document.createElement('img');
    image.className = '_nou_replay_emoji';
    image.alt =
      pickString(emoji, 'image', 'accessibility', 'accessibilityData', 'label') ||
      pickString(emoji, 'shortcuts', 0) ||
      pickString(emoji, 'emojiId') ||
      '';
    const src = chatReplayImageUrl(pick(emoji, 'image', 'thumbnails', 0, 'url'));
    if (src) {
      image.src = src;
      element.append(image);
    } else {
      element.append(document.createTextNode(image.alt));
    }
  }
}

function renderMessage(message: ReplayMessage) {
  const row = document.createElement('article');
  row.className = '_nou_replay_message';
  const photo = chatReplayImageUrl(pick(message.authorPhoto, 'thumbnails', 0, 'url'));
  if (photo) {
    const image = document.createElement('img');
    image.className = '_nou_replay_avatar';
    image.src = photo;
    image.alt = '';
    row.append(image);
  }
  const body = document.createElement('span');
  const timestamp = document.createElement('small');
  appendText(timestamp, message.timestampText);
  body.append(timestamp);
  const author = document.createElement('strong');
  appendText(author, message.authorName);
  body.append(author);
  appendText(body, message.purchaseAmountText);
  appendText(body, message.headerSubtext);
  appendText(body, message.message);
  const stickers = pickArray(message.sticker, 'thumbnails');
  const sticker = chatReplayImageUrl(pick(stickers[stickers.length - 1], 'url'));
  if (sticker) {
    const image = document.createElement('img');
    image.className = '_nou_replay_sticker';
    image.src = sticker;
    image.alt =
      pickString(message.sticker, 'accessibility', 'accessibilityData', 'label') || 'Paid sticker';
    body.append(image);
  }
  row.append(body);
  return row;
}

export function mountChatReplay(container: Element, videoId: string, initialContinuation: string) {
  const panel = document.createElement('section');
  panel.className = '_nou_replay_panel';
  const header = document.createElement('header');
  const title = document.createElement('span');
  title.textContent = 'Chat replay';
  header.append(title);
  const close = container.querySelector('._nou_livechat_close');
  if (close) header.append(close);
  const status = document.createElement('p');
  status.textContent = 'Loading...';
  const retry = document.createElement('button');
  retry.className = '_nou_replay_retry';
  retry.textContent = 'Retry';
  retry.hidden = true;
  const list = document.createElement('section');
  list.className = '_nou_replay_messages';
  list.setAttribute('aria-label', 'Chat replay messages');
  panel.append(header, status, retry, list);
  container.prepend(panel);

  const buffer = new ChatReplayBuffer();
  let continuation = initialContinuation;
  let seekContinuation = initialContinuation;
  let controller = new AbortController();
  let revision = 0;
  let stopped = false;
  let loading = false;
  let failed = false;
  let lastTime: number | undefined;
  let prefetchThresholdMsec = 5000;
  let nextRequestAt = 0;
  let hasLoaded = false;
  const rows = new Map<string, { message: ReplayMessage; element: HTMLElement }>();

  function redraw() {
    const atBottom = list.scrollHeight - list.scrollTop - list.clientHeight < 40;
    const listTop = list.getBoundingClientRect().top;
    // A row still shown holds its place while older rows go.
    const anchor = atBottom
      ? undefined
      : Array.from(rows.entries()).find(
          ([id, row]) =>
            buffer.messages.has(id) && row.element.getBoundingClientRect().bottom > listTop,
        );
    const anchorTop = anchor?.[1].element.getBoundingClientRect().top;
    for (const [id, row] of rows) {
      if (!buffer.messages.has(id)) {
        row.element.remove();
        rows.delete(id);
      }
    }
    let previous: HTMLElement | undefined;
    for (const [id, message] of buffer.messages) {
      let row = rows.get(id);
      if (!row || row.message !== message) {
        const element = renderMessage(message);
        row?.element.replaceWith(element);
        row = { message, element };
        rows.set(id, row);
      }
      const expected = previous ? previous.nextElementSibling : list.firstElementChild;
      if (row.element !== expected) list.insertBefore(row.element, expected);
      previous = row.element;
    }
    status.hidden = !failed && buffer.messages.size > 0;
    if (atBottom) {
      list.scrollTop = list.scrollHeight;
    } else if (anchor && anchorTop !== undefined) {
      const element = rows.get(anchor[0])?.element;
      if (element) list.scrollTop += element.getBoundingClientRect().top - anchorTop;
    }
  }

  function reset() {
    controller.abort();
    controller = new AbortController();
    revision++;
    loading = false;
    failed = false;
    nextRequestAt = 0;
    hasLoaded = false;
    continuation = seekContinuation;
    buffer.clear();
    list.replaceChildren();
    rows.clear();
    status.hidden = false;
    status.textContent = 'Loading...';
    retry.hidden = true;
  }

  async function tick() {
    const player = getPlayer();
    if (stopped || !player || readVideo(player)?.id !== videoId) return;
    const time = player.getCurrentTime?.();
    if (typeof time !== 'number' || !Number.isFinite(time)) return;
    const rate = getVideoElement(player)?.playbackRate || 1;
    if (
      lastTime !== undefined &&
      (time < lastTime - 1 || time > lastTime + Math.max(5, rate * 2))
    ) {
      reset();
    }
    lastTime = time;
    if (buffer.advance(time)) redraw();
    if (loading || failed || !continuation || performance.now() < nextRequestAt) return;
    if (
      hasLoaded &&
      (player.getPlayerState?.() !== PlayerState.playing ||
        !shouldPrefetchChatReplay(time, buffer.bufferedUntil, rate, prefetchThresholdMsec))
    ) {
      return;
    }
    loading = true;
    const requestRevision = revision;
    try {
      const response = await request(
        'live_chat/get_live_chat_replay',
        {
          continuation,
          currentPlayerState: { playerOffsetMs: String(Math.floor(time * 1000)) },
        },
        controller.signal,
      );
      if (stopped || requestRevision !== revision) return;
      const chat = pick(response, 'continuationContents', 'liveChatContinuation');
      if (!isObject(chat)) throw new Error('Chat replay is unavailable');
      const actions = pickArray(chat, 'actions');
      buffer.append(actions);
      const continuations = pickArray(chat, 'continuations');
      const next = pick(
        continuations.find((item) => pick(item, 'liveChatReplayContinuationData')),
        'liveChatReplayContinuationData',
      );
      const seek = pick(
        continuations.find((item) => pick(item, 'playerSeekContinuationData')),
        'playerSeekContinuationData',
      );
      continuation = pickString(next, 'continuation') || '';
      seekContinuation = pickString(seek, 'continuation') || initialContinuation;
      prefetchThresholdMsec = Number(pick(next, 'timeUntilLastMessageMsec') ?? 5000);
      // Quiet stretches back off in wall-clock time, whatever the playback speed.
      nextRequestAt = performance.now() + chatReplayPollDelay(actions);
      hasLoaded = true;
      if (buffer.advance(player.getCurrentTime?.() ?? time)) redraw();
      status.hidden = buffer.messages.size > 0;
      status.textContent = 'No messages at this point in the video.';
    } catch {
      if (stopped || requestRevision !== revision) return;
      failed = true;
      status.hidden = false;
      status.textContent = 'Unable to load chat replay.';
      retry.hidden = false;
    } finally {
      if (requestRevision === revision) loading = false;
    }
  }

  retry.onclick = () => {
    reset();
    void tick();
  };
  const video = getVideoElement(getPlayer());
  const onSeek = () => {
    lastTime = undefined;
    reset();
    void tick();
  };
  video?.addEventListener('seeked', onSeek);
  const timer = setInterval(() => void tick(), 500);
  void tick();
  return () => {
    stopped = true;
    controller.abort();
    clearInterval(timer);
    video?.removeEventListener('seeked', onSeek);
  };
}
