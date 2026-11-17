import { commands } from '@raygun/bindings';
import type { PlaybackControl, PlaybackState, PlayContext, QueueSnapshot } from '@raygun/bindings';
import { useMemo } from 'react';

import { useChipEnabled } from '../chips/useChips';
import { unwrapResult } from '../lib/commandResult';
import { usePlayerSelect, useQueueSelect } from './usePlayback';

/** The shuffle button's states: off, shuffle, and Smart Shuffle (a Smart Chip). */
export type ShuffleMode = 'off' | 'shuffle' | 'smart';

export const shuffleModeOf = (state: PlaybackState | undefined): ShuffleMode =>
  !state?.shuffle ? 'off' : state.smartShuffle ? 'smart' : 'shuffle';

/**
 * What a press asks for: off → shuffle → Smart Shuffle (while its chip is installed and on) → off.
 * Shuffle off ends Smart Shuffle too.
 */
export const nextShuffle = (mode: ShuffleMode, smartAvailable: boolean): PlaybackControl => {
  if (mode === 'off') return { type: 'setShuffle', enabled: true };
  if (mode === 'shuffle' && smartAvailable) return { type: 'setSmartShuffle', enabled: true };
  return { type: 'setShuffle', enabled: false };
};

export const useShuffleMode = () => {
  const mode = usePlayerSelect(shuffleModeOf);
  const smartAvailable = useChipEnabled('smartShuffle');
  return { mode, next: nextShuffle(mode, smartAvailable) };
};

const NONE: string[] = [];
const suggestedOf = (queue: QueueSnapshot | undefined) => queue?.suggested ?? NONE;
const contextOf = (queue: QueueSnapshot | undefined) => queue?.context ?? null;

/** The queue's songs Smart Shuffle added (by item id), for their sparkle mark. */
export const useSuggestedIds = (): ReadonlySet<string> => {
  // Equal lists keep their identity (`useQueueSelect`), so the set changes only with them.
  const ids = useQueueSelect(suggestedOf);
  return useMemo(() => new Set(ids), [ids]);
};

/** Whether the queue plays from a saved playlist (Add to this playlist goes there). */
export const playsFromPlaylist = (context: PlayContext | null) =>
  context?.kind === 'playlist' && context.to === '/library/playlist' && !!context.search.id;

export const usePlaysFromPlaylist = () => playsFromPlaylist(useQueueSelect(contextOf));

/** "Not for me": the song leaves, and Smart Shuffle never suggests it for this list again. */
export const notForMe = (itemId: string) => unwrapResult(commands.smartShuffleNotForMe(itemId));

/** "Add to this playlist": the song joins the playlist and becomes one of its own. */
export const addToThisPlaylist = (itemId: string) =>
  unwrapResult(commands.smartShuffleAddToList(itemId));
