import type { CollectionLength } from '@raygun/bindings';
import type { TFunction } from 'i18next';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { useTrackLengths } from './useTrackLengths';

const MINUTE_MS = 60_000;

/** What's known of a collection's length: the items with a length, and how many items it has. */
export type LengthSummary = { knownMs: number; knownCount: number; total: number };

/** A song as far as its length goes. */
export type LengthSong = {
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
};

/** Sums the known lengths; `total` is how many items there are (defaults to the lengths given). */
export const summarizeLengths = (
  lengths: readonly (number | null | undefined)[],
  total = lengths.length,
): LengthSummary => {
  let knownMs = 0;
  let knownCount = 0;
  for (const length of lengths) {
    if (length && length > 0) {
      knownMs += length;
      knownCount += 1;
    }
  }
  return { knownMs, knownCount, total };
};

/** A playlist summary's length, as Rust sums it. */
export const playlistLengthSummary = (
  length: CollectionLength | undefined,
  total: number,
): LengthSummary | null =>
  length ? { knownMs: length.knownSecs * 1000, knownCount: length.knownCount, total } : null;

const unit = (locale: string, name: 'hour' | 'minute', value: number) =>
  new Intl.NumberFormat(locale, { style: 'unit', unit: name, unitDisplay: 'short' }).format(value);

/** Hours and minutes ("2 hr 37 min"), or minutes alone under an hour ("42 min"). */
export const formatHoursMinutes = (ms: number, locale: string) => {
  const minutes = Math.max(1, Math.round(ms / MINUTE_MS));
  const hours = Math.floor(minutes / 60);
  if (hours === 0) return unit(locale, 'minute', minutes);
  const rest = minutes % 60;
  return rest === 0
    ? unit(locale, 'hour', hours)
    : `${unit(locale, 'hour', hours)} ${unit(locale, 'minute', rest)}`;
};

/**
 * The length to show, or `null` when nothing is known. With every length known it is exact; with
 * at least half known it is estimated from their average ("about …"); with fewer it is what the
 * known ones add up to ("…+").
 */
export const describeLength = (
  summary: LengthSummary | null,
  t: TFunction,
  locale: string,
): string | null => {
  if (!summary || summary.knownCount === 0 || summary.knownMs <= 0) return null;
  const { knownMs, knownCount, total } = summary;
  if (knownCount >= total) return formatHoursMinutes(knownMs, locale);
  if (knownCount * 2 >= total) {
    // Rounded to 5 minutes: it's an estimate.
    const estimate = (knownMs / knownCount) * total;
    const rounded = Math.max(5, Math.round(estimate / (5 * MINUTE_MS)) * 5) * MINUTE_MS;
    return t('collectionLength.about', { length: formatHoursMinutes(rounded, locale) });
  }
  return t('collectionLength.atLeast', { length: formatHoursMinutes(knownMs, locale) });
};

/** "42 songs · 2 hr 37 min": the parts that are there, joined. */
export const joinCount = (...parts: (string | null | undefined | false)[]) =>
  parts.filter(Boolean).join(' · ');

/** The length of a summary in the app's language. */
export const useLengthLabel = (summary: LengthSummary | null): string | null => {
  const { t, i18n } = useTranslation();
  return describeLength(summary, t, i18n.language);
};

/**
 * A song list's length, filling the lengths songs lack the way the song table does (M46.26), so
 * the label grows as lengths arrive. `total` is how many songs the list has, when more than given.
 */
export const useSongsLength = (songs: readonly LengthSong[], total?: number): string | null => {
  const rows = useMemo(
    () =>
      songs.map((song, index) => ({
        rowKey: String(index),
        title: song.title,
        artist: song.artist,
        album: song.album,
        durationMs: song.durationMs,
      })),
    [songs],
  );
  const filled = useTrackLengths(rows);
  const summary = useMemo(
    () =>
      summarizeLengths(
        filled.map((row) => row.durationMs),
        Math.max(total ?? 0, filled.length),
      ),
    [filled, total],
  );
  return useLengthLabel(summary);
};
