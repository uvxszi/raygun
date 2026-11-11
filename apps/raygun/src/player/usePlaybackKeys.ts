import { useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { usePlayerUiStore } from './playerUiStore';
import { positionAt } from './useLivePosition';
import { NEXT_REPEAT, pressPlayPause, sendControl, shownState } from './usePlayback';

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

export const typing = (target: EventTarget | null) =>
  target instanceof HTMLInputElement ||
  target instanceof HTMLTextAreaElement ||
  target instanceof HTMLSelectElement ||
  (target instanceof HTMLElement && target.isContentEditable);

export type PlaybackAction = Exclude<(typeof PLAYBACK_SHORTCUTS)[number][1], 'help'>;

/** Runs one playback action against the current state (keys, the app menu; M46.8, M46.9). */
export const runPlaybackAction = (action: PlaybackAction, client: QueryClient) => {
  // The shown player (another device's while playing there), where it is now, in the seek
  // slider's quarter seconds.
  const state = shownState(client);
  const ui = usePlayerUiStore.getState();
  const position = Math.floor(positionAt(state, performance.now()) / 250) * 250;
  const volume = state?.volume ?? 1;
  switch (action) {
    case 'playPause':
      return pressPlayPause(client, { type: 'toggle' });
    case 'next':
      return sendControl({ type: 'next' });
    case 'previous':
      return sendControl({ type: 'previous' });
    case 'louder':
      return sendControl({ type: 'setVolume', volume: Math.min(1, volume + 0.1) });
    case 'quieter':
      return sendControl({ type: 'setVolume', volume: Math.max(0, volume - 0.1) });
    case 'forward':
      return sendControl({ type: 'seek', positionMs: position + 5000 });
    case 'back':
      return sendControl({ type: 'seek', positionMs: Math.max(0, position - 5000) });
    case 'mute':
      if (volume > 0) ui.setUnmutedVolume(volume);
      return sendControl({ type: 'setVolume', volume: volume > 0 ? 0 : ui.unmutedVolume || 1 });
    case 'shuffle':
      return sendControl({ type: 'setShuffle', enabled: !state?.shuffle });
    case 'repeat':
      return sendControl({ type: 'setRepeat', mode: NEXT_REPEAT[state?.repeat ?? 'off'] });
    case 'lyrics':
      return ui.openSheet({ view: 'lyrics' });
    case 'queue':
      return ui.togglePanel('queue');
    case 'player':
      return ui.sheetOpen ? ui.closeSheet() : ui.openSheet();
  }
};

/** The action a key press asks for, if any. */
const actionFor = (event: KeyboardEvent, onButton: boolean): PlaybackAction | 'help' | null => {
  const key = event.key;
  if (key === ' ' && !onButton && !event.ctrlKey) return 'playPause';
  if (event.ctrlKey && key === 'ArrowRight') return 'next';
  if (event.ctrlKey && key === 'ArrowLeft') return 'previous';
  if (event.ctrlKey && key === 'ArrowUp') return 'louder';
  if (event.ctrlKey && key === 'ArrowDown') return 'quieter';
  if (event.shiftKey && key === 'ArrowRight') return 'forward';
  if (event.shiftKey && key === 'ArrowLeft') return 'back';
  if (event.ctrlKey || event.shiftKey) return key === '?' ? 'help' : null;
  const letters: Record<string, PlaybackAction | 'help'> = {
    m: 'mute',
    s: 'shuffle',
    r: 'repeat',
    l: 'lyrics',
    q: 'queue',
    p: 'player',
    '?': 'help',
  };
  return letters[key.toLowerCase()] ?? null;
};

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
      const action = actionFor(event, event.target instanceof HTMLButtonElement);
      if (!action) return;
      event.preventDefault();
      if (action === 'help') onHelp();
      else void runPlaybackAction(action, client);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [client, onHelp, enabled]);
};
