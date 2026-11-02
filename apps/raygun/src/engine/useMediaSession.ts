import { useEffect } from 'react';

import { usePlatform } from '../lib/usePlatform';
import { isActive, sendControl, useNowPlaying } from '../player/usePlayback';

/**
 * The system's media controls on desktop (M46.8): the webview's Media Session shows the song in
 * Windows' media overlay and lets the keyboard's media keys play, pause, skip and seek. Android
 * has its own (Media3's session), so this stays off there.
 */
export const useMediaSession = () => {
  const platform = usePlatform();
  const { state, item } = useNowPlaying();
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
    if (!enabled || !session || typeof MediaMetadata === 'undefined') return;
    session.metadata = item
      ? new MediaMetadata({
          title: item.title,
          artist: item.artist ?? '',
          album: item.album ?? '',
          artwork: item.artworkUrl ? [{ src: item.artworkUrl, sizes: '512x512' }] : [],
        })
      : null;
  }, [enabled, session, item]);

  const playing = isActive(state);
  const duration = (state?.durationMs ?? 0) / 1000;
  const position = (state?.positionMs ?? 0) / 1000;
  useEffect(() => {
    if (!enabled || !session) return;
    session.playbackState = item ? (playing ? 'playing' : 'paused') : 'none';
    if (duration > 0 && position <= duration) {
      try {
        session.setPositionState({ duration, position, playbackRate: state?.rate ?? 1 });
      } catch {
        // A position past the end while a song changes.
      }
    }
  }, [enabled, session, item, playing, duration, position, state?.rate]);
};
