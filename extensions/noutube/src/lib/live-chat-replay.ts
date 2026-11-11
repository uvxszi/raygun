// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From lib/live-chat-replay.ts.

import { isObject, pick, pickArray, pickString } from './json';

/** A chat message as YouTube's renderers carry it; the text fields are YouTube's text nodes. */
export type ReplayMessage = {
  id: string;
  authorExternalChannelId?: string;
  authorName?: unknown;
  authorPhoto?: unknown;
  message?: unknown;
  headerSubtext?: unknown;
  purchaseAmountText?: unknown;
  timestampText?: unknown;
  sticker?: unknown;
};

const messageRenderers = [
  'liveChatTextMessageRenderer',
  'liveChatPaidMessageRenderer',
  'liveChatPaidStickerRenderer',
  'liveChatMembershipItemRenderer',
  'liveChatViewerEngagementMessageRenderer',
  'liveChatSponsorshipsGiftRedemptionAnnouncementRenderer',
];

/** An https picture address; protocol-relative ones get https, anything else is refused. */
export function chatReplayImageUrl(value: unknown): string | undefined {
  if (typeof value !== 'string') return undefined;
  const url = value.startsWith('//') ? `https:${value}` : value;
  return url.startsWith('https://') ? url : undefined;
}

export function chatReplayPollDelay(actions: unknown[] = []) {
  return actions.some((action) => pickArray(action, 'replayChatItemAction', 'actions').length)
    ? 1000
    : 5000;
}

// YouTube gives a threshold in video time for fetching ahead, not a delay before the next
// request. Allow at least two seconds of wall-clock time at the current rate.
export function shouldPrefetchChatReplay(
  time: number,
  bufferedUntil: number,
  rate: number,
  thresholdMsec: number,
) {
  return bufferedUntil - time <= Math.max(thresholdMsec / 1000, rate * 2);
}

const asMessage = (item: unknown): ReplayMessage | undefined => {
  for (const renderer of messageRenderers) {
    const message = pick(item, renderer);
    if (isObject(message) && typeof message.id === 'string' && message.id) {
      return message as ReplayMessage;
    }
  }
  return undefined;
};

// An answer can hold messages ahead of playback. They wait here, so pausing the video also
// pauses the chat; a seek drops them all.
export class ChatReplayBuffer {
  private pending: { time: number; actions: unknown[] }[] = [];
  readonly messages = new Map<string, ReplayMessage>();
  bufferedUntil = 0;

  clear() {
    this.pending = [];
    this.messages.clear();
    this.bufferedUntil = 0;
  }

  append(actions: unknown[] = []) {
    for (const action of actions) {
      const replay = pick(action, 'replayChatItemAction');
      if (!isObject(replay)) continue;
      const time = Number(replay.videoOffsetTimeMsec) / 1000;
      if (!Number.isFinite(time)) continue;
      this.bufferedUntil = Math.max(this.bufferedUntil, time);
      this.pending.push({ time, actions: pickArray(replay, 'actions') });
    }
    this.pending.sort((first, second) => first.time - second.time);
  }

  advance(time: number) {
    let changed = false;
    for (let next = this.pending[0]; next && next.time <= time; next = this.pending[0]) {
      this.pending.shift();
      for (const action of next.actions) {
        const item =
          pick(action, 'addChatItemAction', 'item') ||
          pick(action, 'replaceChatItemAction', 'replacementItem');
        if (item) {
          const message = asMessage(item);
          const replacedId = pickString(action, 'replaceChatItemAction', 'targetItemId');
          if (message) {
            if (replacedId && this.messages.has(replacedId) && replacedId !== message.id) {
              // Map.set keeps the order only when the key stays the same.
              const entries = Array.from(this.messages, ([id, value]): [string, ReplayMessage] =>
                id === replacedId ? [message.id, message] : [id, value],
              );
              this.messages.clear();
              for (const [id, value] of entries) this.messages.set(id, value);
            } else {
              this.messages.set(message.id, message);
            }
            changed = true;
          } else if (replacedId) {
            // YouTube can replace a removed message with a placeholder.
            changed = this.messages.delete(replacedId) || changed;
          }
        }
        const removedId =
          pickString(action, 'markChatItemAsDeletedAction', 'targetItemId') ||
          pickString(action, 'removeChatItemAction', 'targetItemId');
        if (removedId) changed = this.messages.delete(removedId) || changed;
        const authorId =
          pickString(action, 'markChatItemsByAuthorAsDeletedAction', 'externalChannelId') ||
          pickString(action, 'removeChatItemByAuthorAction', 'externalChannelId');
        if (authorId) {
          for (const [id, message] of this.messages) {
            if (message.authorExternalChannelId === authorId) {
              changed = this.messages.delete(id) || changed;
            }
          }
        }
      }
      while (this.messages.size > 100) {
        const oldest = this.messages.keys().next().value;
        if (oldest === undefined) break;
        this.messages.delete(oldest);
      }
    }
    return changed;
  }
}

/** The replay's first continuation, when the watch page's answer offers a chat replay. */
export function getChatReplay(response: unknown): string | undefined {
  const chat = pick(
    response,
    'contents',
    'twoColumnWatchNextResults',
    'conversationBar',
    'liveChatRenderer',
  );
  const continuation = pickString(
    chat,
    'continuations',
    0,
    'reloadContinuationData',
    'continuation',
  );
  return pick(chat, 'isReplay') ? continuation : undefined;
}
