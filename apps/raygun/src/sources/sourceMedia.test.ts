import { addonMedia, mediaOf, mediaOfAll } from './sourceMedia';

describe('what sources are for', () => {
  it('counts details, charts, lyrics and subtitles as data too', () => {
    expect(mediaOf(['music.metadata', 'music.tags'])).toEqual(['audio', 'data']);
    expect(mediaOf(['music.charts'])).toEqual(['audio', 'data']);
    expect(mediaOf(['music.stream'])).toEqual(['audio']);
    expect(mediaOf(['stremio.subtitles'])).toEqual(['video', 'data']);
    expect(mediaOf(['manga.source'])).toEqual(['manga']);
    expect(mediaOf([])).toEqual(['other']);
  });

  it('leaves sources that play music out of Data, though they search through metadata', () => {
    expect(mediaOf(['music.metadata', 'music.stream'])).toEqual(['audio']);
    expect(mediaOf(['music.metadata', 'music.stream', 'video.source'])).toEqual(['audio', 'video']);
    expect(mediaOf(['music.metadata'])).toEqual(['audio', 'data']);
    expect(mediaOf(['music.metadata', 'music.stream', 'lyrics'])).toEqual(['audio', 'data']);
  });

  it('puts a repository under what each of its sources is for', () => {
    expect(mediaOfAll([['music.metadata', 'music.stream'], ['music.charts']])).toEqual([
      'audio',
      'data',
    ]);
    expect(mediaOfAll([['music.metadata', 'music.stream'], ['manga.source']])).toEqual([
      'audio',
      'manga',
    ]);
    expect(mediaOfAll([])).toEqual(['other']);
  });

  it('counts Stremio addons with subtitles or details as data', () => {
    expect(addonMedia(['subtitles'])).toEqual(['video', 'data']);
    expect(addonMedia(['catalog', 'meta'])).toEqual(['video', 'data']);
    expect(addonMedia(['stream'])).toEqual(['video']);
  });
});
