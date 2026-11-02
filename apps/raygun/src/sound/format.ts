import { formatTime } from '../player/formatTime';

/** Tenths of a dB as `+3.5 dB`. */
export const formatDb = (tenths: number) =>
  `${tenths > 0 ? '+' : ''}${(tenths / 10).toFixed(1)} dB`;

/** `1:23.4` from ms, with tenths so trims can be exact. */
export const formatTrim = (ms: number) => {
  const tenths = Math.floor((ms % 1000) / 100);
  return `${formatTime(ms)}.${tenths}`;
};

/** Reads `1:23`, `1:23.4`, `83.4` or `1:02:03` as ms; null when it isn't a time. */
export const parseTrim = (text: string): number | null => {
  const trimmed = text.trim();
  if (!/^\d+(:\d{1,2}){0,2}(\.\d+)?$/.test(trimmed)) return null;
  const [whole = '', fraction = ''] = trimmed.split('.');
  const seconds = whole
    .split(':')
    .map(Number)
    .reduce((total, part) => total * 60 + part, 0);
  return Math.round((seconds + Number(`0.${fraction || '0'}`)) * 1000);
};
