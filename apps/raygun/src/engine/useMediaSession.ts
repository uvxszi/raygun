import type { QueueItem } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { usePlatform } from '../lib/usePlatform';
import { isActive, sendControl, shownQueue, shownState } from '../player/usePlayback';

/** What the session follows in the query cache. */
const FOLLOWED = new Set(['playbackState', 'queue', 'connect']);

/**
 * The system's media controls on desktop (M46.8): the webview's Media Session shows the song in
 * Windows' media overlay and lets the keyboard's media keys play, pause, skip and seek. Android
 * has its own (Media3's session), so this stays off there.
 *
 * It follows the player from the query cache, not through React: it runs in the app shell, and a
 * re-render there for every position report would redraw the whole shell.
 */
export const useMediaSession = () => {
  const platform = usePlatform();
  const client = useQueryClient();
  const session = typeof navigator !== 'undefined' ? navigator.mediaSession : undefined;
  const enabled =
    !!session && platform !== undefined && platform !== 'android' && platform !== 'ios';

  useEffect(() => {
    if (!enabled || !session) return;
    const handlers: [MediaSessionAction, MediaSessionActionHandler][] = [
      ['play', () => void sendControl({ type: 'play' })],
      ['pause', () => void sendControl({ type: 'pause' })],
      ['nexttrack', () => void sendControl({ type: 'next' })],
      ['previoustrack', () => void sendControl({ type: 'previous' })],
      [
        'seekto',
        (details) => void sendControl({ type: 'seek', positionMs: (details.seekTime ?? 0) * 1000 }),
      ],
    ];
    for (const [action, handler] of handlers) {
      try {
        session.setActionHandler(action, handler);
      } catch {
        // Not every webview knows every action.
      }
    }
    return () => {
      for (const [action] of handlers) {
        try {
          session.setActionHandler(action, null);
        } catch {
          // As above.
        }
      }
    };
  }, [enabled, session]);

  useEffect(() => {
    if (!enabled || !session) return;
    let shownItem: QueueItem | undefined | null = null;
    const sync = () => {
      const state = shownState(client);
      const item = shownQueue(client)?.items.find((each) => each.id === state?.currentItemId);
      if (item !== shownItem && typeof MediaMetadata !== 'undefined') {
        shownItem = item;
        session.metadata = item
          ? new MediaMetadata({
              title: item.title,
              artist: item.artist ?? '',
              album: item.album ?? '',
              artwork: item.artworkUrl ? [{ src: item.artworkUrl, sizes: '512x512' }] : [],
            })
          : null;
      }
      session.playbackState = item ? (isActive(state) ? 'playing' : 'paused') : 'none';
      const duration = (state?.durationMs ?? 0) / 1000;
      const position = (state?.positionMs ?? 0) / 1000;
      if (duration > 0 && position <= duration) {
        try {
          // The system moves the position on at this rate between reports.
          session.setPositionState({ duration, position, playbackRate: state?.rate || 1 });
        } catch {
          // A position past the end while a song changes.
        }
      }
    };
    sync();
    return client.getQueryCache().subscribe((event) => {
      if (event.type === 'updated' && FOLLOWED.has(String(event.query.queryKey[0]))) sync();
    });
  }, [enabled, session, client]);
};
