import type { MusicAlbum } from '@raygun/bindings';

import { sortReleases } from './releaseSort';
import { dragOrder } from './tableView';
import type { TableTrack } from './tableView';

const row = (title: string): TableTrack => ({
  rowKey: title,
  trackKey: `x|${title}`,
  title,
  artist: 'x',
  album: null,
  durationMs: null,
  addedAt: null,
  playCount: 0,
  artworkUrl: null,
  input: { title, artist: 'x', album: null, durationMs: null, artworkUrl: null, source: null },
});
const keys = (rows: TableTrack[]) => rows.map((each) => each.rowKey);

describe('dragOrder (M46.44)', () => {
  const all = ['a', 'b', 'c', 'd', 'e'].map(row);

  it('moves a row up before, and down after, the row it was dropped on', () => {
    expect(keys(dragOrder(all, all, 3, 1))).toEqual(['a', 'd', 'b', 'c', 'e']);
    expect(keys(dragOrder(all, all, 0, 2))).toEqual(['b', 'c', 'a', 'd', 'e']);
  });

  it('keeps rows a filter hides in their places', () => {
    // Only a, c and e show; e dropped on c lands before c, b and d stay put.
    const shown = [all[0]!, all[2]!, all[4]!];
    expect(keys(dragOrder(all, shown, 2, 1))).toEqual(['a', 'b', 'e', 'c', 'd']);
  });

  it('starts from the sorted order, which becomes the saved one', () => {
    const sorted = [...all].reverse();
    expect(keys(dragOrder(sorted, sorted, 0, 4))).toEqual(['d', 'c', 'b', 'a', 'e']);
  });
});

describe('sortReleases (M46.45)', () => {
  const album = (id: string, year: number | null, trackCount: number | null) =>
    ({ id, title: id, year, trackCount }) as MusicAlbum;
  const list = [album('B', 2001, 10), album('A', null, 3), album('C', 2020, null)];
  const ids = (sort: Parameters<typeof sortReleases>[1]) =>
    sortReleases(list, sort).map((each) => each.id);

  it('sorts by date either way, by title, and by songs, unknowns last', () => {
    expect(ids('newest')).toEqual(['C', 'B', 'A']);
    expect(ids('oldest')).toEqual(['B', 'C', 'A']);
    expect(ids('title')).toEqual(['A', 'B', 'C']);
    expect(ids('tracks')).toEqual(['B', 'A', 'C']);
  });
});
