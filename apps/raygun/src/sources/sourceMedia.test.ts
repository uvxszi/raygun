import { addonMedia, mediaOf } from './sourceMedia';

describe('what sources are for', () => {
  it('counts details, charts, lyrics and subtitles as data too', () => {
    expect(mediaOf(['music.metadata', 'music.tags'])).toEqual(['audio', 'data']);
    expect(mediaOf(['music.charts'])).toEqual(['audio', 'data']);
    expect(mediaOf(['music.stream'])).toEqual(['audio']);
    expect(mediaOf(['stremio.subtitles'])).toEqual(['video', 'data']);
    expect(mediaOf(['manga.source'])).toEqual(['manga']);
    expect(mediaOf([])).toEqual(['other']);
  });

  it('counts Stremio addons with subtitles or details as data', () => {
    expect(addonMedia(['subtitles'])).toEqual(['video', 'data']);
    expect(addonMedia(['catalog', 'meta'])).toEqual(['video', 'data']);
    expect(addonMedia(['stream'])).toEqual(['video']);
  });
});
