import type { PictureLook } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { PlayerPrefs } from '../../settings/playerOptions';
import { PrefChoice, PrefGroup, PrefSlider, PrefSwitch } from '../../settings/prefs';
import { ADJUSTMENT_LABEL, LOOK_LABEL, SMART_AUDIO } from './effectLabels';
import { ADJUSTMENTS, LOOKS, shownValue } from './effectParams';

type Props = {
  prefs: PlayerPrefs;
  set: (change: Partial<PlayerPrefs>) => void;
  busy: boolean;
};

/**
 * Settings → Video player → Effects: the picture's effects and smart audio for titles without
 * their own (all off unless changed). A video's own changes stay with its title.
 */
export const EffectsSettings: FC<Props> = ({ prefs, set, busy }) => {
  const { t } = useTranslation();
  const picture = prefs.pictureEffects;
  const audio = prefs.smartAudio;
  return (
    <>
      <PrefGroup
        title={t('settings.player.effects.picture')}
        summary={t('settings.player.effects.pictureHint')}
      >
        <PrefChoice<PictureLook>
          title={t('settings.player.effects.look')}
          value={picture.look}
          disabled={busy}
          options={LOOKS.map((look) => ({ value: look, label: t(LOOK_LABEL[look]) }))}
          onChange={(look) => set({ pictureEffects: { ...picture, look } })}
        />
        {ADJUSTMENTS.map((adjustment) => (
          <PrefSlider
            key={adjustment.key}
            title={t(ADJUSTMENT_LABEL[adjustment.key])}
            value={picture[adjustment.key]}
            min={adjustment.min}
            max={adjustment.max}
            step={5}
            format={(value) => shownValue(adjustment.key, value)}
            disabled={busy}
            onChange={(value) => set({ pictureEffects: { ...picture, [adjustment.key]: value } })}
          />
        ))}
        <PrefSwitch
          title={t('videoPlayer.effectSmoothMotion')}
          summary={t('videoPlayer.effectSmoothMotionHint')}
          checked={picture.smoothMotion}
          disabled={busy}
          onChange={(on) => set({ pictureEffects: { ...picture, smoothMotion: on } })}
        />
      </PrefGroup>
      <PrefGroup
        title={t('settings.player.effects.sound')}
        summary={t('settings.player.effects.soundHint')}
      >
        {SMART_AUDIO.map(({ key, label, hint }) => (
          <PrefSwitch
            key={key}
            title={t(label)}
            summary={t(hint)}
            checked={audio[key]}
            disabled={busy}
            onChange={(on) => set({ smartAudio: { ...audio, [key]: on } })}
          />
        ))}
      </PrefGroup>
    </>
  );
};
