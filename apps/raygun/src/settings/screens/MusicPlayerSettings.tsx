import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlayerViewSettings } from '../../player/PlayerViewSettings';
import { useIsDesktop } from '../../shell/useIsDesktop';
import { usePanelSizes } from '../../shell/usePanelSizes';
import { SoundSettings } from '../../sound/SoundSettings';
import { AudioQualitySettings } from '../AudioQualitySettings';
import { PrefGroup, PrefSlider, PrefSwitch } from '../prefs';
import { useSettings, useUpdateSetting } from '../useSettings';

/** Matches `PanelPrefs::validate` in raygun-model (the side panel's Next in queue). */
const NEXT_UP_ROWS = { min: 1, max: 10 };

/**
 * Settings → Music player: quality, what happens on start, the queue, the full player's look,
 * then the sound (equalizer, effects, levelling, crossfade). Also the music player's own sheet.
 */
export const MusicPlayerSettings: FC = () => {
  const { t } = useTranslation();
  const data = useSettings().data;
  const update = useUpdateSetting();
  const desktop = useIsDesktop();
  const { panels, save } = usePanelSizes();
  if (!data) return null;
  return (
    <>
      <AudioQualitySettings />
      <PrefGroup>
        <PrefSwitch
          title={t('settings.resumeOnStart')}
          checked={data.resumeOnStart ?? true}
          onChange={(value) => update.mutate({ key: 'resumeOnStart', value })}
        />
        <PrefSwitch
          title={t('settings.keepMusicPlaying')}
          checked={data.keepMusicPlaying ?? false}
          onChange={(value) => update.mutate({ key: 'keepMusicPlaying', value })}
        />
      </PrefGroup>
      {/* The side panel's list; phones show the queue on its own page. */}
      {desktop && (
        <PrefGroup title={t('settings.musicQueue')}>
          <PrefSlider
            title={t('settings.nextUpRows')}
            value={panels.nextUpRows || NEXT_UP_ROWS.min}
            min={NEXT_UP_ROWS.min}
            max={NEXT_UP_ROWS.max}
            step={1}
            format={String}
            onChange={(nextUpRows) => save({ nextUpRows })}
          />
        </PrefGroup>
      )}
      <PlayerViewSettings title={t('player.view.musicTitle')} />
      <SoundSettings />
    </>
  );
};
