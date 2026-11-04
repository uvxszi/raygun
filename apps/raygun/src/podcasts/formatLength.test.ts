import { runtimeMs } from './formatLength';

describe('runtimeMs', () => {
  it('reads the lengths directories write', () => {
    // As the Internet Archive and LibriVox give them.
    expect(runtimeMs('13:06:44')).toBe(47_204_000);
    expect(runtimeMs('16:31.09')).toBe(59_469_000);
    expect(runtimeMs('41:37')).toBe(2_497_000);
    expect(runtimeMs('0:45.59')).toBe(2_759_000);
  });

  it('leaves out what it can’t read', () => {
    expect(runtimeMs(null)).toBeNull();
    expect(runtimeMs('')).toBeNull();
    expect(runtimeMs('about an hour')).toBeNull();
    expect(runtimeMs('45')).toBeNull();
    expect(runtimeMs('0:00:00')).toBeNull();
  });
});
