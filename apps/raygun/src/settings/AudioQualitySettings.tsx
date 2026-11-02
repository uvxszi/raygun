import type { AudioQuality, AudioQualityPrefs } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefChoice, PrefGroup } from './prefs';
import type { PrefOption } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const LEVELS: AudioQuality[] = ['low', 'normal', 'high', 'veryHigh', 'best'];

const DEFAULTS: AudioQualityPrefs = { streaming: 'high', cellular: 'normal', download: 'veryHigh' };

/**
 * Audio quality as Spotify offers it (M46.18): one choice for streaming, one for streaming on
 * mobile data, one for downloads. Sources that have a song at several bitrates give the nearest;
 * the rest play what they have.
 */
export const AudioQualitySettings: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  if (!settings.data) return null;
  const prefs = settings.data.audioQuality ?? DEFAULTS;
  const options: PrefOption<AudioQuality>[] = LEVELS.map((level) => ({
    value: level,
    label: t(`settings.audioQuality.levels.${level}`),
    hint: t(`settings.audioQuality.hints.${level}`),
  }));
  const set = (change: Partial<AudioQualityPrefs>) =>
    update.mutate({ key: 'audioQuality', value: { ...prefs, ...change } });
  return (
    <PrefGroup title={t('settings.audioQuality.title')}>
      <PrefChoice<AudioQuality>
        title={t('settings.audioQuality.streaming')}
        summary={t('settings.audioQuality.streamingHint')}
        value={prefs.streaming}
        options={options}
        disabled={update.isPending}
        onChange={(streaming) => set({ streaming })}
      />
      <PrefChoice<AudioQuality>
        title={t('settings.audioQuality.cellular')}
        summary={t('settings.audioQuality.cellularHint')}
        value={prefs.cellular}
        options={options}
        disabled={update.isPending}
        onChange={(cellular) => set({ cellular })}
      />
      <PrefChoice<AudioQuality>
        title={t('settings.audioQuality.download')}
        summary={t('settings.audioQuality.downloadHint')}
        value={prefs.download}
        options={options}
        disabled={update.isPending}
        onChange={(download) => set({ download })}
      />
    </PrefGroup>
  );
};
