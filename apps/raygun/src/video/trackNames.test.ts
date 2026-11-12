import {
  channelsName,
  distinctTracks,
  fileLanguage,
  mentions,
  trackFacts,
  withMarks,
} from './trackNames';
import type { TrackMark } from './trackNames';

const numbered = (name: string, number: number) => `${name} ${number}`;
const WORDS: Record<TrackMark, string> = {
  forced: 'Forced',
  sdh: 'SDH',
  commentary: 'Commentary',
  description: 'Audio description',
};
const word = (mark: TrackMark) => WORDS[mark];

describe('track names', () => {
  it('numbers tracks in one language, where each comes from under it', () => {
    expect(
      distinctTracks(
        [
          { name: 'English', detail: 'In the video' },
          { name: 'English', detail: 'OpenSubtitles v3' },
          { name: 'English', detail: 'OpenSubtitles v3' },
          { name: 'Français', detail: 'OpenSubtitles v3' },
        ],
        numbered,
      ),
    ).toEqual([
      { name: 'English', detail: 'In the video' },
      { name: 'English 2', detail: 'OpenSubtitles v3' },
      { name: 'English 3', detail: 'OpenSubtitles v3' },
      { name: 'Français', detail: 'OpenSubtitles v3' },
    ]);
  });

  it('tells tracks apart by what their flags say before numbering them', () => {
    const names = distinctTracks(
      [
        { name: 'English' },
        { name: 'English', marks: ['SDH'] },
        { name: 'English', marks: ['Forced'] },
        { name: 'English (Forced)', marks: ['Forced', 'SDH'] },
      ],
      numbered,
    ).map((track) => track.name);
    expect(names).toEqual([
      'English',
      'English (SDH)',
      'English (Forced)',
      'English (Forced, SDH)',
    ]);
  });

  it('never numbers a name into one another track has', () => {
    const names = distinctTracks(
      [{ name: 'English' }, { name: 'English' }, { name: 'English 2' }],
      numbered,
    ).map((track) => track.name);
    expect(names).toEqual(['English', 'English 3', 'English 2']);
  });

  it('leaves out a detail that only says the name again', () => {
    expect(distinctTracks([{ name: 'film.srt', detail: 'film.srt' }], numbered)).toEqual([
      { name: 'film.srt', detail: null },
    ]);
  });

  it('names a file by its language, the file under it', () => {
    expect(trackFacts({ key: 'a', label: 'Film.en.srt', language: 'en' }, word)).toEqual({
      name: 'English',
      marks: [],
      detail: 'Film.en.srt',
    });
    expect(trackFacts({ key: 'b', label: 'film.srt', language: null }, word)).toMatchObject({
      name: 'film.srt',
    });
    // An addon's bare code reads as the language, the addon under it.
    expect(
      trackFacts({ key: 'c', label: 'eng', language: 'eng' }, word, 'OpenSubtitles v3'),
    ).toEqual({ name: 'English', marks: [], detail: 'OpenSubtitles v3' });
  });

  it('adds only the marks a name doesn’t say already', () => {
    expect(
      trackFacts({ key: 'a', label: 'English CC', language: 'en', sdh: true }, word).marks,
    ).toEqual([]);
    expect(
      trackFacts({ key: 'b', label: 'English', language: 'en', forced: true, sdh: true }, word)
        .marks,
    ).toEqual(['Forced', 'SDH']);
    // Whole words only: "Hindi" doesn't say "HI".
    expect(mentions('Hindi', 'hi')).toBe(false);
    expect(mentions('English [HI]', 'hi')).toBe(true);
    expect(withMarks('English (Signs)', ['SDH'])).toBe('English (Signs, SDH)');
  });

  it('reads a file’s language and an audio track’s channels', () => {
    expect(fileLanguage('Film.2019.en.srt')).toBe('en');
    expect(fileLanguage('Film.English.srt')).toBe('en');
    expect(fileLanguage('film.srt')).toBeNull();
    expect(channelsName('2')).toBe('Stereo');
    expect(channelsName('6')).toBe('5.1');
    expect(channelsName('16/JOC')).toBe('16 ch');
    expect(channelsName(undefined)).toBeNull();
  });
});
