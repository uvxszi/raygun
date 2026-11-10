import { drawerCover, drawerListWidth } from './drawerSizes';

describe('Quick access drawer sizes', () => {
  it('takes the list’s width from the drawer’s: 18 rem at most with names, 5.5 rem without', () => {
    // A 393 px phone: 18 rem fits, less the list's 1 rem of padding.
    expect(drawerListWidth(true, 393, 16)).toBe(272);
    // A 320 px one: 82% of it.
    expect(drawerListWidth(true, 320, 16)).toBeCloseTo(246.4);
    expect(drawerListWidth(false, 393, 16)).toBe(80);
  });

  it('fills a row with covers only, and gives covers beside names a fifth of the list', () => {
    expect(drawerCover(false, 80, 16)).toBe(68);
    expect(drawerCover(true, 272, 16)).toBe(57);
    // Never smaller than 3 rem nor larger than 4.5 rem beside names, at any interface size.
    expect(drawerCover(true, 150, 16)).toBe(48);
    expect(drawerCover(true, 600, 16)).toBe(72);
    expect(drawerCover(true, 272, 20)).toBe(60);
  });
});
