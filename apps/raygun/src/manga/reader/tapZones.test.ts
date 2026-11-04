import type { TapCell } from '@raygun/bindings';

import { gridOf, TAP_PRESETS, tapAction, toGrid } from './tapZones';

const prefs = (tapZones: 'auto' | 'kindle' | 'custom', webtoon?: TapCell[]) => ({
  tapZones,
  tapGridPaged: toGrid(TAP_PRESETS.edge),
  tapGridWebtoon: toGrid(webtoon ?? TAP_PRESETS.kindle),
});

describe('tap zones', () => {
  it('uses the sides for pages and the L shape for the strip by default', () => {
    expect(gridOf(prefs('auto'), 'paged')).toEqual(TAP_PRESETS.sides);
    expect(gridOf(prefs('auto'), 'webtoon')).toEqual(TAP_PRESETS.lShaped);
    expect(gridOf(prefs('kindle'), 'webtoon')).toEqual(TAP_PRESETS.kindle);
  });

  it('uses each mode’s own grid when custom', () => {
    expect(gridOf(prefs('custom'), 'paged')).toEqual(TAP_PRESETS.edge);
    expect(gridOf(prefs('custom'), 'webtoon')).toEqual(TAP_PRESETS.kindle);
    // A broken stored grid falls back to the kind's default.
    expect(gridOf(prefs('custom', ['menu']), 'webtoon')).toEqual(TAP_PRESETS.lShaped);
  });

  it('turns pages with the sides, mirrored for right to left books', () => {
    const sides = TAP_PRESETS.sides;
    expect(tapAction(sides, 'none', 'pagedLtr', 0.1, 0.5)).toBe('back');
    expect(tapAction(sides, 'none', 'pagedLtr', 0.9, 0.5)).toBe('forward');
    expect(tapAction(sides, 'none', 'pagedLtr', 0.5, 0.5)).toBe('menu');
    expect(tapAction(sides, 'none', 'pagedRtl', 0.1, 0.5)).toBe('forward');
    expect(tapAction(sides, 'none', 'pagedRtl', 0.9, 0.5)).toBe('back');
  });

  it('scrolls the strip with the L shape', () => {
    const shape = TAP_PRESETS.lShaped;
    expect(tapAction(shape, 'none', 'webtoon', 0.5, 0.1)).toBe('back');
    expect(tapAction(shape, 'none', 'webtoon', 0.1, 0.5)).toBe('back');
    expect(tapAction(shape, 'none', 'webtoon', 0.9, 0.5)).toBe('forward');
    expect(tapAction(shape, 'none', 'webtoon', 0.5, 0.9)).toBe('forward');
    expect(tapAction(shape, 'none', 'webtoon', 0.5, 0.5)).toBe('menu');
  });

  it('mirrors the zones and can turn them off', () => {
    expect(tapAction(TAP_PRESETS.lShaped, 'vertical', 'webtoon', 0.5, 0.1)).toBe('forward');
    expect(tapAction(TAP_PRESETS.lShaped, 'horizontal', 'webtoon', 0.1, 0.5)).toBe('forward');
    expect(tapAction(TAP_PRESETS.sides, 'both', 'pagedLtr', 0.1, 0.5)).toBe('forward');
    expect(tapAction(TAP_PRESETS.edge, 'none', 'pagedLtr', 0.5, 0.9)).toBe('back');
    expect(tapAction(TAP_PRESETS.edge, 'none', 'pagedLtr', 0.1, 0.1)).toBe('forward');
    expect(tapAction(TAP_PRESETS.kindle, 'none', 'pagedLtr', 0.5, 0.1)).toBe('menu');
    expect(tapAction(TAP_PRESETS.off, 'none', 'pagedLtr', 0.1, 0.5)).toBe('menu');
  });

  it('mirrors custom grids and gives their jumps', () => {
    const grid: TapCell[] = [
      'firstPage',
      'nothing',
      'lastPage',
      'previousChapter',
      'menu',
      'nextChapter',
      'left',
      'previous',
      'right',
    ];
    expect(tapAction(grid, 'none', 'pagedLtr', 0.1, 0.1)).toBe('firstPage');
    expect(tapAction(grid, 'horizontal', 'pagedLtr', 0.1, 0.1)).toBe('lastPage');
    expect(tapAction(grid, 'vertical', 'pagedLtr', 0.5, 0.9)).toBe('nothing');
    expect(tapAction(grid, 'none', 'webtoon', 0.9, 0.5)).toBe('nextChapter');
    expect(tapAction(grid, 'none', 'pagedRtl', 0.1, 0.9)).toBe('forward');
  });
});
