import type { PlayerLayout, TextAlign } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSwitch } from '../settings/prefs';
import {
  LYRICS_SIZES,
  PLAYER_LAYOUTS,
  PLAYER_WIDTHS,
  TEXT_ALIGNS,
  usePlayerView,
} from './playerView';

/**
 * How the full music player looks (M46.7): side by side or stacked, its width, the lyrics' size
 * and alignment, and the blurred artwork behind it. In Settings → Player and the player's own
 * Layout button.
 */
export const PlayerViewSettings: FC<{ title?: string }> = ({ title }) => {
  const { t } = useTranslation();
  const { view, set, busy } = usePlayerView();
  return (
    <PrefGroup title={title ?? t('player.view.title')}>
      <PrefChoice<PlayerLayout>
        title={t('player.view.layout')}
        value={view.layout}
        disabled={busy}
        options={PLAYER_LAYOUTS.map((value) => ({
          value,
          label: t(`player.view.layouts.${value}`),
          hint: t(`player.view.layoutHints.${value}`),
        }))}
        onChange={(layout) => set({ layout })}
      />
      <PrefChoice<number>
        title={t('player.view.width')}
        summary={t('player.view.widthHint')}
        value={view.width}
        disabled={busy}
        options={PLAYER_WIDTHS.map((value) => ({
          value,
          label:
            value === 0 ? t('reader.display.byScreen') : t('reader.display.percent', { n: value }),
        }))}
        onChange={(width) => set({ width })}
      />
      <PrefChoice<number>
        title={t('player.view.lyricsSize')}
        value={view.lyricsSize}
        disabled={busy}
        options={LYRICS_SIZES.map((value) => ({
          value,
          label: value === 0 ? t('reader.display.byScreen') : t('player.view.px', { n: value }),
        }))}
        onChange={(lyricsSize) => set({ lyricsSize })}
      />
      <PrefChoice<TextAlign>
        title={t('player.view.lyricsAlign')}
        value={view.lyricsAlign}
        disabled={busy}
        options={TEXT_ALIGNS.map((value) => ({
          value,
          label: t(`player.view.aligns.${value}`),
        }))}
        onChange={(lyricsAlign) => set({ lyricsAlign })}
      />
      <PrefSwitch
        title={t('player.view.background')}
        checked={view.background}
        disabled={busy}
        onChange={(background) => set({ background })}
      />
    </PrefGroup>
  );
};
