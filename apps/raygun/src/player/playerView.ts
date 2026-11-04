import type { PlayerLayout, PlayerViewPrefs, TextAlign } from '@raygun/bindings';
import type { CSSProperties } from 'react';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

export type PlayerView = Required<PlayerViewPrefs>;

/** Matches `PlayerViewPrefs::default()` in raygun-model. */
export const PLAYER_VIEW_DEFAULTS: PlayerView = {
  layout: 'auto',
  coverOffset: 0,
  widthOffset: 0,
  lyricsOffset: 0,
  lyricsAlign: 'left',
  background: true,
};

export const PLAYER_LAYOUTS: PlayerLayout[] = ['auto', 'sideBySide', 'stacked'];
/** How far the cover and the player's width move from their size by screen, in percent
 * (`PLAYER_PERCENT_OFFSETS` in raygun-model). */
export const PERCENT_OFFSETS = { min: -40, max: 40, step: 5 } as const;
/** How far lyrics text moves from its size by screen, in px (`LYRICS_OFFSETS`). */
export const LYRICS_OFFSETS = { min: -5, max: 10, step: 1 } as const;
export const TEXT_ALIGNS: TextAlign[] = ['left', 'center'];

/** The full player's layout settings (M46.7), whole, and a way to change some of them. */
export const usePlayerView = () => {
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const view: PlayerView = { ...PLAYER_VIEW_DEFAULTS, ...settings?.playerView };
  const set = (patch: Partial<PlayerView>) =>
    update.mutate({ key: 'playerView', value: { ...view, ...patch } });
  return { view, set, busy: update.isPending };
};

/**
 * The lyrics' size and alignment from Settings → Player: text sized by its column, moved by the
 * chosen offset (styles.css, `.lyric-text`).
 */
export const useLyricsLook = () => {
  const { view } = usePlayerView();
  return {
    align: view.lyricsAlign,
    style: { '--lyrics-offset': `${view.lyricsOffset / 16}rem` } as CSSProperties,
  };
};
