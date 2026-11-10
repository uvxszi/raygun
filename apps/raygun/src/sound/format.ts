import { formatTime } from '../player/formatTime';

/** Tenths of a dB as `+3.5 dB`. */
export const formatDb = (tenths: number) =>
  `${tenths > 0 ? '+' : ''}${(tenths / 10).toFixed(1)} dB`;

/** `1:23.4` from ms, with tenths so trims can be exact. */
export const formatTrim = (ms: number) => {
  const tenths = Math.floor((ms % 1000) / 100);
  return `${formatTime(ms)}.${tenths}`;
};

/** The furthest a lyrics offset moves the lines either way, ms. */
const MAX_OFFSET_MS = 600_000;

/** A lyrics offset as its field shows it, in seconds: `+0.4`, `-1.25`, `0.0`. */
export const formatOffset = (ms: number) => {
  const seconds = (Math.abs(ms) / 1000).toFixed(3).replace(/0+$/, '').replace(/\.$/, '.0');
  return `${ms > 0 ? '+' : ms < 0 ? '-' : ''}${seconds}`;
};

/**
 * A typed lyrics offset in ms: seconds with decimals (`1.5`, `-0.25`, `+2`, `1,5`, `0.4 s`) or
 * milliseconds (`250 ms`), kept within ten minutes either way; null when it isn't one.
 */
export const parseOffset = (text: string): number | null => {
  const match = /^([+\-−]?)\s*(\d+(?:[.,]\d*)?|[.,]\d+)\s*(ms|s)?$/i.exec(text.trim());
  if (!match) return null;
  const [, sign = '', amount = '', unit = 's'] = match;
  const value = Number(amount.replace(',', '.'));
  if (!Number.isFinite(value)) return null;
  const ms = Math.round(unit.toLowerCase() === 'ms' ? value : value * 1000);
  const signed = sign === '+' || sign === '' ? ms : -ms;
  return Math.max(-MAX_OFFSET_MS, Math.min(MAX_OFFSET_MS, signed));
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
