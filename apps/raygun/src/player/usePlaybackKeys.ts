import type { PlaybackState, RepeatMode } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { usePlayerUiStore } from './playerUiStore';
import { sendControl } from './usePlayback';

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

/** The shortcuts, for the help sheet (M46.8); keys as shown to people. */
export const PLAYBACK_SHORTCUTS = [
  ['Space', 'playPause'],
  ['Ctrl + →', 'next'],
  ['Ctrl + ←', 'previous'],
  ['Shift + →', 'forward'],
  ['Shift + ←', 'back'],
  ['Ctrl + ↑', 'louder'],
  ['Ctrl + ↓', 'quieter'],
  ['M', 'mute'],
  ['S', 'shuffle'],
  ['R', 'repeat'],
  ['L', 'lyrics'],
  ['Q', 'queue'],
  ['P', 'player'],
  ['?', 'help'],
] as const;

const typing = (target: EventTarget | null) =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLElement && target.isContentEditable);

/**
 * Playback from the keyboard, anywhere (M46.8): Space, Ctrl/Shift with the arrows, and single
 * letters, never while typing or when a focused button would take the key itself.
 */
export const usePlaybackKeys = (onHelp: () => void, enabled = true) => {
  const client = useQueryClient();
  useEffect(() => {
    if (!enabled) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || typing(event.target) || event.altKey || event.metaKey) return;
      const state = client.getQueryData<PlaybackState>(['playbackState']);
      const ui = usePlayerUiStore.getState();
      const position = state?.positionMs ?? 0;
      const volume = state?.volume ?? 1;
      const onButton = event.target instanceof HTMLButtonElement;
      const key = event.key;
      const act = (() => {
        if (key === ' ' && !onButton && !event.ctrlKey)
          return () => sendControl({ type: 'toggle' });
        if (event.ctrlKey && key === 'ArrowRight') return () => sendControl({ type: 'next' });
        if (event.ctrlKey && key === 'ArrowLeft') return () => sendControl({ type: 'previous' });
        if (event.ctrlKey && key === 'ArrowUp')
          return () => sendControl({ type: 'setVolume', volume: Math.min(1, volume + 0.1) });
        if (event.ctrlKey && key === 'ArrowDown')
          return () => sendControl({ type: 'setVolume', volume: Math.max(0, volume - 0.1) });
        if (event.shiftKey && key === 'ArrowRight')
          return () => sendControl({ type: 'seek', positionMs: position + 5000 });
        if (event.shiftKey && key === 'ArrowLeft')
          return () => sendControl({ type: 'seek', positionMs: Math.max(0, position - 5000) });
        if (event.ctrlKey || event.shiftKey) return key === '?' ? onHelp : null;
        switch (key.toLowerCase()) {
          case 'm':
            return () => {
              if (volume > 0) ui.setUnmutedVolume(volume);
              void sendControl({
                type: 'setVolume',
                volume: volume > 0 ? 0 : ui.unmutedVolume || 1,
              });
            };
          case 's':
            return () => sendControl({ type: 'setShuffle', enabled: !state?.shuffle });
          case 'r':
            return () =>
              sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[state?.repeat ?? 'off'] });
          case 'l':
            return () => ui.openSheet({ view: 'lyrics' });
          case 'q':
            return () => ui.togglePanel('queue');
          case 'p':
            return () => (ui.sheetOpen ? ui.closeSheet() : ui.openSheet());
          case '?':
            return onHelp;
          default:
            return null;
        }
      })();
      if (!act) return;
      event.preventDefault();
      void act();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [client, onHelp, enabled]);
};
