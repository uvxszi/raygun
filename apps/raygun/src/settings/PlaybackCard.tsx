import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from './useSettings';

/** Matches `VIDEO_QUALITIES` in raygun-model; empty means the highest available. */
const QUALITIES = [1080, 720, 480, 360];

/** Settings → Playback: which video quality to pick first. */
export const PlaybackCard: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();

  if (!settings.data) {
    return null;
  }

  return (
    <section
      data-testid="playback-settings-card"
      className="bg-surface flex w-full flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.playback')}</h2>
      <label className="flex flex-col gap-1.5">
        <span className="font-medium">{t('settings.videoQuality')}</span>
        <select
          value={settings.data.preferredVideoQuality ?? ''}
          disabled={updateSetting.isPending}
          onChange={(event) =>
            updateSetting.mutate({
              key: 'preferredVideoQuality',
              value: event.target.value ? Number(event.target.value) : null,
            })
          }
          className="bg-background rounded-lg px-3 py-2"
        >
          <option value="">{t('settings.videoQualityHighest')}</option>
          {QUALITIES.map((height) => (
            <option key={height} value={height}>
              {t('settings.videoQualityUpTo', { height })}
            </option>
          ))}
        </select>
      </label>
      <p className="text-muted text-xs">{t('settings.videoQualityHint')}</p>
      {updateSetting.isError && (
        <p role="alert" className="text-primary">
          {t('settings.saveFailed', { message: updateSetting.error.message })}
        </p>
      )}
    </section>
  );
};
