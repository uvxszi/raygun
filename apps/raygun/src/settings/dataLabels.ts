import type { BackupManifest, BackupPlace, DataCategory, DataCount } from '@raygun/bindings';
import type { TFunction } from 'i18next';

import { formatBytes } from '../lib/formatBytes';

/** Query keys: where backups go, and what each category holds here. */
export const DESTINATIONS = ['backups', 'destinations'] as const;
export const DATA_USAGE = ['dataUsage'] as const;

/** Settings → Data lists categories in this order: the library, music, the app, then files. */
export const CATEGORY_ORDER: DataCategory[] = [
  'comics',
  'books',
  'videos',
  'podcasts',
  'audiobooks',
  'websites',
  'playlists',
  'pins',
  'history',
  'settings',
  'sources',
  'searches',
  'aiModels',
  'downloads',
  'localFiles',
  'caches',
];

/** Categories a backup can hold (caches are only cleared). */
export const BACKUP_CATEGORIES = CATEGORY_ORDER.filter((category) => category !== 'caches');

export const sortCategories = <T extends { category: DataCategory }>(list: T[]): T[] =>
  [...list].sort((a, b) => CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category));

/** "12 titles · 340 chapters · 1.2 GB"; empty when there's nothing. */
export const countsLine = (t: TFunction, counts: DataCount[], bytes = 0): string =>
  [
    ...counts
      .filter((count) => count.count > 0)
      .map((count) => t(`data.items.${count.item}`, { count: count.count })),
    ...(bytes > 0 ? [formatBytes(bytes)] : []),
  ].join(' · ');

/** Where a backup is, as shown: Raygun's own folder by name, other places as they're called. */
export const folderLabel = (
  t: TFunction,
  place: BackupPlace,
  folder: string,
  fallback = '',
): string => (place === 'app' ? t('data.places.app') : folder || fallback);

/** Who made a backup: "Pixel 4 · Raygun 1.2.0", or an earlier Raygun's. */
export const madeBy = (t: TFunction, manifest: BackupManifest): string =>
  manifest.appVersion
    ? t('data.backups.madeBy', {
        device: manifest.deviceName || t('data.backups.someDevice'),
        version: manifest.appVersion,
      })
    : t('data.backups.madeEarlier');
