import type { TFunction } from 'i18next';

/** What Rust calls the words a transcription heard (M46.45). */
export const TRANSCRIBED = 'transcribed';

/** A lyrics source's name to show. */
export const lyricsSourceName = (provider: string, t: TFunction) =>
  provider === TRANSCRIBED ? t('lyricsEdit.transcribedSource') : provider;
