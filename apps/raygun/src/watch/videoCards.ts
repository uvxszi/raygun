import type { VideoCardStyle } from '@raygun/bindings';
import { useSyncExternalStore } from 'react';

import { useDataSaving, useNetworkStatus } from '../network/useNetworkMode';
import { useSettings } from '../settings/useSettings';
import { useDeviceState, useEffectiveMotion } from '../shell/deviceState';
import { useVideoPlayerStore } from '../video/videoPlayerStore';

/** How video titles show as cards (`Settings::videoCards.style`): 16:9 unless chosen otherwise. */
export const useVideoCardStyle = (): VideoCardStyle =>
  useSettings().data?.videoCards?.style ?? 'landscape';

/** Grid columns for 16:9 cards: wider than posters' (as video sites'). */
export const LANDSCAPE_GRID = {
  gridTemplateColumns: 'repeat(auto-fill, minmax(min(100%, 13rem), 1fr))',
};

const HOVER_QUERY = '(hover: hover) and (pointer: fine)';

const subscribeHover = (listener: () => void) => {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  const query = window.matchMedia(HOVER_QUERY);
  query.addEventListener('change', listener);
  return () => query.removeEventListener('change', listener);
};

const hoverNow = () =>
  typeof window !== 'undefined' && Boolean(window.matchMedia?.(HOVER_QUERY).matches);

/** A mouse that can rest on things (desktop), rather than a finger. */
export const usePointerHovers = () => useSyncExternalStore(subscribeHover, hoverNow, () => false);

/** What starts a card's preview: a mouse resting on it, or the card in the screen's middle. */
export type PreviewTrigger = 'hover' | 'center';

/**
 * How cards play their previews now, or `null` for none (`Settings::videoCards.previews`): Auto
 * on hover with a mouse only, Always also in the middle of a phone's screen. None while Offline
 * Mode, Data Saver (trailers), battery saver or motion Off holds, nor while a video plays.
 */
export const usePreviewTrigger = (): PreviewTrigger | null => {
  const settings = useSettings().data;
  const mode = settings?.videoCards?.previews ?? 'auto';
  const motion = useEffectiveMotion(settings?.motion);
  const { device } = useDeviceState();
  const offline = useNetworkStatus().offline;
  const saving = useDataSaving('noTrailers');
  const hovers = usePointerHovers();
  const playing = useVideoPlayerStore((state) => Boolean(state.request || state.session));
  if (mode === 'off' || offline || saving || device.powerSave || motion === 'off' || playing) {
    return null;
  }
  if (hovers) return 'hover';
  return mode === 'always' ? 'center' : null;
};
