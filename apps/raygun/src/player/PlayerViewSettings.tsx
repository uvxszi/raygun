import type { PlayerLayout, TextAlign } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup, PrefSize, PrefSwitch } from '../settings/prefs';
import {
  LYRICS_OFFSETS,
  PERCENT_OFFSETS,
  PLAYER_LAYOUTS,
  TEXT_ALIGNS,
  usePlayerView,
} from './playerView';

/**
 * How the full music player looks (M46.7): side by side or stacked, and its sizes, each starting
 * from what the screen gives it and moved from there (the cover, the player's width, the
 * lyrics), the lyrics' alignment and the blurred artwork behind it. In Settings → Player and the
 * player's own Layout button.
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
      <PrefSize
        title={t('player.view.cover')}
        value={view.coverOffset}
        {...PERCENT_OFFSETS}
        unit="%"
        disabled={busy}
        onChange={(coverOffset) => set({ coverOffset })}
      />
      <PrefSize
        title={t('player.view.width')}
        value={view.widthOffset}
        {...PERCENT_OFFSETS}
        unit="%"
        disabled={busy}
        onChange={(widthOffset) => set({ widthOffset })}
      />
      <PrefSize
        title={t('player.view.lyricsSize')}
        value={view.lyricsOffset}
        {...LYRICS_OFFSETS}
        unit="px"
        disabled={busy}
        onChange={(lyricsOffset) => set({ lyricsOffset })}
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
