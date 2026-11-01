import type { TFunction } from 'i18next';

/** How a network permission reads: `*` (Shosetsu scripts) means any public website. */
export const hostLabel = (host: string, t: TFunction) =>
  host === '*' ? t('extensions.anyWebsite') : host;
