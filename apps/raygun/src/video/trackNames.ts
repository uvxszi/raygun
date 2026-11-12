/**
 * Names that tell a video's tracks apart in the player's panels, as the Android player names them
 * (`PlayerTrackNames.kt`): a track's name gets what its flags add ("English (Forced)",
 * "English (SDH)"), names still alike are numbered ("English", "English 2"), and where each comes
 * from (an addon, a file, the video) goes under it.
 */

import { useTranslation } from 'react-i18next';

import { languageCode, shownTrackName } from './trackPicking';
import type { TrackOption } from './trackPicking';

/** What a track is besides its language. */
export type TrackMark = 'forced' | 'sdh' | 'commentary' | 'description';

/** The words a name says a mark with already, besides the viewer's own word for it. */
const MARK_WORDS: Record<TrackMark, string[]> = {
  forced: ['forced'],
  sdh: ['sdh', 'cc', 'hi', 'hoh'],
  commentary: ['commentary'],
  description: ['description', 'descriptive', 'ad'],
};

/** What a panel knows of a track to name it. */
export type TrackFacts = {
  /** Its name so far: "English", "English (Forced)", "film.srt". */
  name: string;
  /** Words for what else it is, said after the name unless it says them. */
  marks?: string[];
  /** What goes under the name: where it comes from, how far it got, or an audio track's sound. */
  detail?: string | null;
};

/** A track as a panel lists it. */
export type ShownTrack = { name: string; detail: string | null };

/** A subtitle file's own name ("Film.en.srt"), rather than a track's name. */
const FILE_NAME = /\.(srt|vtt|webvtt|ass|ssa|ttml|dfxp|sub|sbv|smi|txt)$/i;

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** Whether `text` holds `word` as a word of its own, in any case. */
export const mentions = (text: string, word: string): boolean => {
  const plain = word.trim();
  if (!plain) return false;
  return new RegExp(`(?<![\\p{L}\\p{N}])${escape(plain)}(?![\\p{L}\\p{N}])`, 'iu').test(text);
};

/** `name` with the `marks` it doesn't say yet, in brackets: "English (Forced)", "English (Signs, SDH)". */
export const withMarks = (name: string, marks: string[] = []): string => {
  const unsaid = [
    ...new Set(marks.map((mark) => mark.trim()).filter((mark) => mark && !mentions(name, mark))),
  ];
  if (unsaid.length === 0) return name;
  const words = unsaid.join(', ');
  return name.endsWith(')') && name.includes(' (')
    ? `${name.slice(0, -1)}, ${words})`
    : `${name} (${words})`;
};

/**
 * Names that tell `tracks` apart, in their order:
 * - each name gets the marks it doesn't say yet ([withMarks]);
 * - names still alike are numbered, the first kept: "English", "English 2", "English 3";
 * - what goes under a name stays, unless it is that name again (a file named by its file alone).
 * `numbered` words a name with its number.
 */
export const distinctTracks = (
  tracks: TrackFacts[],
  numbered: (name: string, number: number) => string,
): ShownTrack[] => {
  const named = tracks.map((track) => withMarks(track.name, track.marks));
  const taken = new Set(named.map((name) => name.toLowerCase()));
  const seen = new Map<string, number>();
  return tracks.map((track, index) => {
    const base = named[index] ?? track.name;
    const key = base.toLowerCase();
    const count = (seen.get(key) ?? 0) + 1;
    seen.set(key, count);
    let name = base;
    if (count > 1) {
      let number = count;
      name = numbered(base, number);
      while (taken.has(name.toLowerCase())) {
        number += 1;
        name = numbered(base, number);
      }
      taken.add(name.toLowerCase());
    }
    const detail = track.detail?.trim();
    return {
      name,
      detail: detail && detail.toLowerCase() !== track.name.toLowerCase() ? detail : null,
    };
  });
};

/** The marks a track's flags give. */
export const marksOf = (option: TrackOption): TrackMark[] => [
  ...(option.forced ? (['forced'] as const) : []),
  ...(option.sdh ? (['sdh'] as const) : []),
  ...(option.commentary ? (['commentary'] as const) : []),
  ...(option.description ? (['description'] as const) : []),
];

/**
 * A track's facts from its label, language and flags (`word` gives a mark's word in the viewer's
 * language), with `detail` under it: a label that is a file's name ("Film.en.srt", a file added
 * here) goes under the language's name as where it comes from.
 */
export const trackFacts = (
  option: TrackOption,
  word: (mark: TrackMark) => string,
  detail?: string | null,
): TrackFacts => {
  const label = option.label.trim();
  const file = FILE_NAME.test(label) ? label : null;
  const code = file ? languageCode(option.language) : null;
  const name = file
    ? code
      ? shownTrackName(code, code)
      : file
    : shownTrackName(label, option.language);
  const marks = [...new Set(marksOf(option))]
    .filter((mark) => ![...MARK_WORDS[mark], word(mark)].some((said) => mentions(name, said)))
    .map(word);
  return { name, marks, detail: detail ?? file };
};

/** The words names are made with: a mark's, and a name with its number ("English 2"). */
export type TrackWords = {
  mark: (mark: TrackMark) => string;
  numbered: (name: string, number: number) => string;
};

const MARK_KEYS: Record<TrackMark, string> = {
  forced: 'videoPlayer.markForced',
  sdh: 'videoPlayer.markSdh',
  commentary: 'videoPlayer.markCommentary',
  description: 'videoPlayer.markDescription',
};

/** The player's words for track names. */
export const useTrackWords = (): TrackWords => {
  const { t } = useTranslation();
  return {
    mark: (mark) => t(MARK_KEYS[mark]),
    numbered: (name, number) => t('videoPlayer.numberedTrack', { n: name, of: number }),
  };
};

/**
 * Names that tell `options` apart, in their order ([distinctTracks]): what each is besides its
 * language, numbers for names still alike, and under each `detail` (where it comes from).
 */
export const nameTracks = <T extends TrackOption>(
  options: T[],
  words: TrackWords,
  detail: (option: T) => string | null | undefined,
): ShownTrack[] =>
  distinctTracks(
    options.map((option) => trackFacts(option, words.mark, detail(option))),
    words.numbered,
  );

/** "Film.en.srt" says its language before its type: `en`. Null when it says none. */
export const fileLanguage = (fileName: string): string | null => {
  const parts = fileName.split('.');
  return parts.length > 2 ? languageCode(parts.at(-2)) : null;
};

/** An HLS track's channels ("2", "6", "16/JOC") as a name: "Stereo", "5.1". */
export const channelsName = (channels: string | null | undefined): string | null => {
  const count = Number.parseInt(channels ?? '', 10);
  if (!Number.isFinite(count) || count <= 0) return null;
  switch (count) {
    case 1:
      return 'Mono';
    case 2:
      return 'Stereo';
    case 6:
      return '5.1';
    case 8:
      return '7.1';
    default:
      return `${count} ch`;
  }
};
