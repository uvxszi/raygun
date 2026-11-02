import type { PlayerLayout, PlayerViewPrefs, TextAlign } from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

export type PlayerView = Required<PlayerViewPrefs>;

/** Matches `PlayerViewPrefs::default()` in raygun-model. */
export const PLAYER_VIEW_DEFAULTS: PlayerView = {
  layout: 'auto',
  width: 0,
  lyricsSize: 0,
  lyricsAlign: 'left',
  background: true,
};

export const PLAYER_LAYOUTS: PlayerLayout[] = ['auto', 'sideBySide', 'stacked'];
/** Player widths offered, in percent of the window (0: by screen). */
export const PLAYER_WIDTHS = [0, 40, 50, 60, 70, 80, 90, 100];
/** Lyrics sizes offered, in px (0: by screen). */
export const LYRICS_SIZES = [0, 16, 20, 24, 28, 32, 40, 48];
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
