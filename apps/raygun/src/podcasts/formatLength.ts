import type { TFunction } from 'i18next';

const MINUTE_MS = 60_000;

/**
 * A book's length as a directory writes it ("13:06:44", "16:31.09", "41:37"), in ms; `null` when
 * it can't be read.
 */
export const runtimeMs = (runtime: string | null) => {
  if (!runtime) return null;
  const parts = runtime.trim().split(/[:.]/).map(Number);
  if (parts.length < 2 || parts.length > 3 || parts.some((part) => !Number.isFinite(part))) {
    return null;
  }
  const seconds = parts.reduce((total, part) => total * 60 + part, 0);
  return seconds > 0 ? seconds * 1000 : null;
};

/** "1 h 5 min" / "42 min". */
export const formatLength = (ms: number, t: TFunction) => {
  const minutes = Math.max(1, Math.round(ms / MINUTE_MS));
  const hours = Math.floor(minutes / 60);
  return hours > 0
    ? t('podcasts.hoursMinutes', { hours, minutes: minutes % 60 })
    : t('podcasts.minutes', { minutes });
};
