import {
  arrangeSources,
  bulkChoice,
  columnsFor,
  gridRows,
  moveFocus,
  readView,
} from './customGrid';
import type { GridSource } from './customGrid';

const source = (id: string, repository: string | null = null, lang: string | null = null) => ({
  id,
  name: `Source ${id}`,
  adult: false,
  icon: null,
  repository,
  lang,
});

const labelOf = (_: unknown, key: string) => key || 'Other';

describe("Custom sources' grid", () => {
  it('fits 4, 6 or 8 tiles a row by the width it has', () => {
    expect(columnsFor(0, 16)).toBe(4);
    expect(columnsFor(447, 16)).toBe(4);
    expect(columnsFor(448, 16)).toBe(6);
    expect(columnsFor(607, 16)).toBe(6);
    expect(columnsFor(608, 16)).toBe(8);
    // A larger interface (a bigger rem) takes more room per tile.
    expect(columnsFor(608, 20)).toBe(6);
  });

  it('switches what is shown, keeping the rest', () => {
    expect(bulkChoice(['a', 'x'], ['a', 'b'], 'all')).toEqual(['a', 'x', 'b']);
    expect(bulkChoice(['a', 'x'], ['a', 'b'], 'none')).toEqual(['x']);
    expect(bulkChoice(['a', 'x'], ['a', 'b'], 'invert')).toEqual(['x', 'b']);
  });

  it('groups by name, many languages and unknown ones last, the ones on first when asked', () => {
    const sources: GridSource[] = [
      source('1', 'Paraline', 'ja'),
      source('2', null, null),
      source('3', 'Keiyoushi', 'multi'),
      source('4', 'Paraline', 'en'),
    ];
    const byRepository = arrangeSources(sources, {
      needle: '',
      grouping: 'repository',
      first: null,
      labelOf,
    });
    expect(byRepository.map((group) => [group.key, group.sources.map((one) => one.id)])).toEqual([
      ['Keiyoushi', ['3']],
      ['Paraline', ['1', '4']],
      ['', ['2']],
    ]);
    const byLanguage = arrangeSources(sources, {
      needle: '',
      grouping: 'language',
      first: new Set(['4']),
      labelOf,
    });
    expect(byLanguage.map((group) => group.key)).toEqual(['en', 'ja', 'multi', '']);
    const flat = arrangeSources(sources, {
      needle: 'source 4',
      grouping: 'none',
      first: new Set(['4']),
      labelOf,
    });
    expect(flat.map((group) => group.sources.map((one) => one.id))).toEqual([['4']]);
  });

  it('moves in reading order across rows, and up and down in the column kept', () => {
    const groups = arrangeSources(
      ['1', '2', '3', '4', '5', '6'].map((id) => source(id, id < '4' ? 'A' : 'B')),
      { needle: '', grouping: 'repository', first: null, labelOf },
    );
    // A: header, [1 2], [3]; B: header, [4 5], [6].
    const rows = gridRows(groups, 2, new Set(), true);
    expect(rows.map((row) => row.kind)).toEqual([
      'group',
      'tiles',
      'tiles',
      'group',
      'tiles',
      'tiles',
    ]);
    expect(moveFocus(rows, { row: 1, col: 1 }, 'ArrowRight', false, 1, 4)).toEqual({
      row: 2,
      col: 0,
    });
    expect(moveFocus(rows, { row: 2, col: 0 }, 'ArrowLeft', false, 0, 4)).toEqual({
      row: 1,
      col: 1,
    });
    // Down from the second column: the next header's switch, then the second column again.
    expect(moveFocus(rows, { row: 2, col: 0 }, 'ArrowDown', false, 1, 4)).toEqual({
      row: 3,
      col: 1,
    });
    expect(moveFocus(rows, { row: 3, col: 1 }, 'ArrowDown', false, 1, 4)).toEqual({
      row: 4,
      col: 1,
    });
    expect(moveFocus(rows, { row: 0, col: 0 }, 'ArrowUp', false, 0, 4)).toBeNull();
    expect(moveFocus(rows, { row: 5, col: 0 }, 'ArrowRight', false, 0, 4)).toBeNull();
    expect(moveFocus(rows, { row: 1, col: 0 }, 'PageDown', false, 0, 4)).toEqual({
      row: 5,
      col: 0,
    });
    expect(moveFocus(rows, { row: 4, col: 1 }, 'End', true, 1, 4)).toEqual({ row: 5, col: 0 });
    expect(moveFocus(rows, { row: 4, col: 1 }, 'Home', false, 1, 4)).toEqual({ row: 4, col: 0 });
    // A folded group has its header only.
    expect(gridRows(groups, 2, new Set(['A']), true).map((row) => row.kind)).toEqual([
      'group',
      'group',
      'tiles',
      'tiles',
    ]);
  });

  it('reads a saved view, leaving out what it does not know', () => {
    localStorage.setItem(
      'raygun.search.customView.novel',
      JSON.stringify({ grouping: 'sideways', onFirst: true, folded: ['A', 3] }),
    );
    expect(readView('novel')).toEqual({ grouping: 'none', onFirst: true, folded: ['A'] });
    localStorage.setItem('raygun.search.customView.novel', 'not json');
    expect(readView('novel')).toBeNull();
    expect(readView('manga')).toBeNull();
  });
});
