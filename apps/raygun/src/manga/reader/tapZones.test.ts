import { resolveZones, tapAction } from './tapZones';

describe('tap zones', () => {
  it('uses the sides for pages and the L shape for the strip by default', () => {
    expect(resolveZones('auto', 'pagedLtr')).toBe('sides');
    expect(resolveZones('auto', 'webtoon')).toBe('lShaped');
    expect(resolveZones('kindle', 'webtoon')).toBe('kindle');
  });

  it('turns pages with the sides, mirrored for right to left books', () => {
    expect(tapAction('auto', 'none', 'pagedLtr', 0.1, 0.5)).toBe('back');
    expect(tapAction('auto', 'none', 'pagedLtr', 0.9, 0.5)).toBe('forward');
    expect(tapAction('auto', 'none', 'pagedLtr', 0.5, 0.5)).toBe('menu');
    expect(tapAction('auto', 'none', 'pagedRtl', 0.1, 0.5)).toBe('forward');
    expect(tapAction('auto', 'none', 'pagedRtl', 0.9, 0.5)).toBe('back');
  });

  it('scrolls the strip with the L shape', () => {
    expect(tapAction('auto', 'none', 'webtoon', 0.5, 0.1)).toBe('back');
    expect(tapAction('auto', 'none', 'webtoon', 0.1, 0.5)).toBe('back');
    expect(tapAction('auto', 'none', 'webtoon', 0.9, 0.5)).toBe('forward');
    expect(tapAction('auto', 'none', 'webtoon', 0.5, 0.9)).toBe('forward');
    expect(tapAction('auto', 'none', 'webtoon', 0.5, 0.5)).toBe('menu');
  });

  it('mirrors the zones and can turn them off', () => {
    expect(tapAction('lShaped', 'vertical', 'webtoon', 0.5, 0.1)).toBe('forward');
    expect(tapAction('lShaped', 'horizontal', 'webtoon', 0.1, 0.5)).toBe('forward');
    expect(tapAction('sides', 'both', 'pagedLtr', 0.1, 0.5)).toBe('forward');
    expect(tapAction('edge', 'none', 'pagedLtr', 0.5, 0.9)).toBe('back');
    expect(tapAction('edge', 'none', 'pagedLtr', 0.1, 0.1)).toBe('forward');
    expect(tapAction('kindle', 'none', 'pagedLtr', 0.5, 0.1)).toBe('menu');
    expect(tapAction('off', 'none', 'pagedLtr', 0.1, 0.5)).toBe('menu');
  });
});
