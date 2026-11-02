import { cuesAt, parseSubtitles, parseTimestamp } from './subtitles';
import { languageCode, matchChoice, pickByLanguage } from './trackPicking';

describe('subtitle files', () => {
  it('reads timestamps in every common form', () => {
    expect(parseTimestamp('00:01:02,345')).toBe(62_345);
    expect(parseTimestamp('01:02.5')).toBe(62_500);
    expect(parseTimestamp('1:00:00.00')).toBe(3_600_000);
    expect(parseTimestamp('nope')).toBeNull();
  });

  it('reads SubRip with markup and Windows line ends', () => {
    const srt =
      '﻿1\r\n00:00:01,000 --> 00:00:03,500\r\n<i>Hello</i> &amp; welcome\r\nSecond line\r\n\r\n2\r\n00:00:04,000 --> 00:00:05,000\r\n{\\an8}Top\r\n';
    const cues = parseSubtitles(srt, 'srt');
    expect(cues).toEqual([
      { startMs: 1_000, endMs: 3_500, lines: ['Hello & welcome', 'Second line'] },
      { startMs: 4_000, endMs: 5_000, lines: ['Top'] },
    ]);
    expect(cuesAt(cues, 2_000)).toEqual(['Hello & welcome', 'Second line']);
    expect(cuesAt(cues, 3_600)).toEqual([]);
  });

  it('reads WebVTT with cue settings and skips its header', () => {
    const vtt =
      'WEBVTT\n\nNOTE a comment\n\nintro\n00:01.000 --> 00:02.000 line:0 align:start\n<v Anna>Hi there</v>\n';
    expect(parseSubtitles(vtt)).toEqual([{ startMs: 1_000, endMs: 2_000, lines: ['Hi there'] }]);
  });

  it('reads ASS dialogue with commas in the text', () => {
    const ass = `[Script Info]
Title: test

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:02.00,0:00:04.50,Default,,0,0,0,,{\\i1}Well, then\\Nwe go
Dialogue: 0,0:00:01.00,0:00:01.50,Default,,0,0,0,,First`;
    expect(parseSubtitles(ass, null)).toEqual([
      { startMs: 1_000, endMs: 1_500, lines: ['First'] },
      { startMs: 2_000, endMs: 4_500, lines: ['Well, then', 'we go'] },
    ]);
  });
});

describe('track picking', () => {
  const tracks = [
    { key: 'a', label: 'English (Signs & Songs)', language: 'eng' },
    { key: 'b', label: 'English Full', language: 'en-US' },
    { key: 'c', label: 'Español', language: 'spa' },
  ];

  it('treats language codes and names alike', () => {
    expect(languageCode('eng')).toBe('en');
    expect(languageCode('en-GB')).toBe('en');
    expect(languageCode('Spanish')).toBe('es');
    expect(languageCode('')).toBeNull();
  });

  it('prefers and skips tracks by words, in the first language that has any', () => {
    expect(pickByLanguage(tracks, ['fr', 'en'], 'full', 'signs')?.key).toBe('b');
    expect(pickByLanguage(tracks, ['en'], '', 'signs')?.key).toBe('b');
    expect(pickByLanguage(tracks, ['en'])?.key).toBe('a');
    expect(pickByLanguage(tracks, ['de'])).toBeNull();
  });

  it('finds a remembered track by label, then language', () => {
    expect(matchChoice(tracks, { label: 'Español', language: null })?.key).toBe('c');
    expect(matchChoice(tracks, { label: 'Gone', language: 'es' })?.key).toBe('c');
    expect(matchChoice(tracks, { label: null, language: 'ja' })).toBeNull();
  });
});
