import { EXTENSION_KINDS, kindsOf } from './catalogFilters';

describe('kindsOf', () => {
  it('puts data sources under Data as well as what they are for', () => {
    expect(kindsOf({ provides: ['music.metadata', 'music.tags'] })).toEqual(['music', 'data']);
    expect(kindsOf({ provides: ['lyrics'] })).toEqual(['lyrics', 'data']);
  });

  it('leaves sources of content out of Data', () => {
    expect(kindsOf({ provides: ['manga.source'] })).toEqual(['manga']);
    expect(kindsOf({ provides: ['music.stream'] })).toEqual(['music']);
    expect(kindsOf({ provides: ['music.metadata', 'music.stream', 'video.source'] })).toEqual([
      'music',
      'video',
    ]);
  });

  it('calls an extension that provides nothing known Other', () => {
    expect(kindsOf({ provides: [] })).toEqual(['other']);
  });

  it('lists Data before Other', () => {
    expect(EXTENSION_KINDS.indexOf('data')).toBeLessThan(EXTENSION_KINDS.indexOf('other'));
  });
});
