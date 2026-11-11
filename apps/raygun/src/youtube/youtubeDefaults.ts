import type { SegmentCategory, SponsorBlockPrefs, YouTubePrefs } from '@raygun/bindings';

/** Matches `SponsorBlockPrefs::default()` in raygun-model. */
export const SPONSORBLOCK_DEFAULTS: Required<SponsorBlockPrefs> = {
  enabled: true,
  sponsor: 'skip',
  selfpromo: 'skip',
  interaction: 'skip',
  intro: 'show',
  outro: 'show',
  preview: 'show',
  musicOfftopic: 'skip',
  filler: 'off',
  trimDownloads: false,
};

/** Matches `YouTubePrefs::default()` in raygun-model. */
export const YOUTUBE_DEFAULTS: Required<YouTubePrefs> = {
  sponsorBlock: SPONSORBLOCK_DEFAULTS,
  originalAudio: true,
};

/** The kinds of parts, in Settings' order (`SegmentCategory::ALL`). */
export const SEGMENT_CATEGORIES: SegmentCategory[] = [
  'sponsor',
  'selfpromo',
  'interaction',
  'intro',
  'outro',
  'preview',
  'musicOfftopic',
  'filler',
];
