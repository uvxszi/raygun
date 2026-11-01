import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from './useSettings';

/** Matches `MANGA_UPDATE_HOURS` in raygun-model. */
const UPDATE_HOURS = [0, 6, 12, 24, 48, 168];

/** Settings → Manga: how often the library is checked for new chapters. */
export const MangaCard: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const updateSetting = useUpdateSetting();

  if (!settings.data) {
    return null;
  }

  return (
    <section
      data-testid="manga-settings-card"
      className="bg-surface flex w-full flex-col gap-3 rounded-xl px-5 py-4 text-sm"
    >
      <h2 className="text-muted">{t('settings.manga')}</h2>
      <label className="flex flex-col gap-1.5">
        <span className="font-medium">{t('settings.mangaUpdates')}</span>
        <select
          value={settings.data.mangaUpdateHours}
          disabled={updateSetting.isPending}
          onChange={(event) =>
            updateSetting.mutate({ key: 'mangaUpdateHours', value: Number(event.target.value) })
          }
          className="bg-background rounded-lg px-3 py-2"
        >
          {UPDATE_HOURS.map((hours) => (
            <option key={hours} value={hours}>
              {t(`settings.updateEvery.${hours}`)}
            </option>
          ))}
        </select>
      </label>
      <p className="text-muted text-xs">{t('settings.mangaUpdatesHint')}</p>
      {updateSetting.isError && (
        <p role="alert" className="text-primary">
          {t('settings.saveFailed', { message: updateSetting.error.message })}
        </p>
      )}
    </section>
  );
};
