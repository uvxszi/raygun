import { countRead, expandUpcE, isDominant, rankCandidates, validCode } from './candidates';
import type { Tally } from './candidates';

describe('barcode candidates', () => {
  it('keeps codes whose check digit is right and drops the rest', () => {
    // EAN-13 (an ISBN) and the same with a digit misread.
    expect(validCode('9780306406157')).toBe('9780306406157');
    expect(validCode('9780306406158')).toBeNull();
    expect(validCode('978-0-306-40615-7')).toBe('9780306406157');
    // UPC-A is written as its EAN-13.
    expect(validCode('720642442517')).toBe('0720642442517');
    expect(validCode('720642442518')).toBeNull();
    // EAN-8.
    expect(validCode('96385074', 'ean_8')).toBe('96385074');
    expect(validCode('96385075', 'ean_8')).toBeNull();
    // Not a product barcode.
    expect(validCode('12345')).toBeNull();
    expect(validCode('https://example.org')).toBeNull();
  });

  it('writes UPC-E codes out as their UPC-A', () => {
    expect(expandUpcE('04252614')).toBe('042100005264');
    expect(validCode('04252614', 'upc_e')).toBe('0042100005264');
    expect(validCode('04252615', 'upc_e')).toBeNull();
  });

  it('counts the same barcode read as UPC-A and EAN-13 once', () => {
    const tally: Tally = new Map();
    expect(countRead(tally, '720642442517')).toBe(1);
    expect(countRead(tally, '0720642442517', 'ean_13')).toBe(2);
    expect(countRead(tally, '0720642442518', 'ean_13')).toBe(0);
    expect(rankCandidates(tally)).toEqual([{ code: '0720642442517', reads: 2 }]);
  });

  it('ranks the codes read most first, at most three, ties by the first read', () => {
    const tally: Tally = new Map();
    [
      '4006381333931',
      '9780306406157',
      '9780306406157',
      '5901234123457',
      '9780306406157',
      '96385074',
      '4006381333931',
    ].forEach((code) => countRead(tally, code));
    expect(rankCandidates(tally)).toEqual([
      { code: '9780306406157', reads: 3 },
      { code: '4006381333931', reads: 2 },
      { code: '5901234123457', reads: 1 },
    ]);
  });

  it('calls a code dominant only when it clearly won', () => {
    expect(isDominant([])).toBe(false);
    expect(isDominant([{ code: 'a', reads: 1 }])).toBe(true);
    expect(
      isDominant([
        { code: 'a', reads: 4 },
        { code: 'b', reads: 2 },
      ]),
    ).toBe(true);
    expect(
      isDominant([
        { code: 'a', reads: 3 },
        { code: 'b', reads: 2 },
      ]),
    ).toBe(false);
    expect(
      isDominant([
        { code: 'a', reads: 2 },
        { code: 'b', reads: 1 },
      ]),
    ).toBe(false);
  });
});
