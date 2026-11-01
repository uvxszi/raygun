import type { TFunction } from 'i18next';

const MINUTE_MS = 60_000;

/** "1 h 5 min" / "42 min". */
export const formatLength = (ms: number, t: TFunction) => {
  const minutes = Math.max(1, Math.round(ms / MINUTE_MS));
  const hours = Math.floor(minutes / 60);
  return hours > 0
    ? t('podcasts.hoursMinutes', { hours, minutes: minutes % 60 })
    : t('podcasts.minutes', { minutes });
};
