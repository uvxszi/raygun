import type { SourceGroup } from './protocol';
import { sourceName, sourceNote, sourcesChecked } from './waiting';

const groups: SourceGroup[] = [
  {
    name: 'Vidstream',
    state: 'ready',
    error: null,
    links: [
      { label: '1080p', current: false, failed: true },
      { label: '720p', current: true, failed: false },
    ],
  },
  {
    name: 'Torrentio',
    state: 'ready',
    error: null,
    links: [{ label: 'Torrentio 4k · The.Show.mkv', current: false, failed: false }],
  },
  { name: 'Slow', state: 'resolving', error: null, links: [] },
];

describe('what the loading ring says', () => {
  it('names a source as the viewer knows it', () => {
    expect(sourceName(groups[0]!, 1)).toBe('Vidstream · 720p');
    expect(sourceName(groups[1]!, 0)).toBe('Torrentio 4k');
  });

  it('counts the sources that answered while a video is found', () => {
    expect(sourcesChecked(groups)).toEqual({ kind: 'checked', answered: 2, total: 3 });
    expect(sourcesChecked(groups.slice(0, 1))).toBeNull();
    expect(sourcesChecked(groups.slice(0, 2)), 'all answered').toBeNull();
  });

  it('says which source is tried, and only its name when it was picked', () => {
    expect(sourceNote(groups, false)).toEqual({
      kind: 'trying',
      number: 2,
      of: 3,
      source: 'Vidstream · 720p',
    });
    expect(sourceNote(groups, true)).toEqual({ kind: 'source', source: 'Vidstream · 720p' });
    expect(sourceNote([], false)).toBeNull();
  });
});
