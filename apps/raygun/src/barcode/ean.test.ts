import { describe, expect, it } from 'vitest';

import { DIGITS, FIRST_DIGIT_PARITY, checkDigitOk, decodeImage, decodeRow } from './ean';

/** Draws an EAN-13 (or EAN-8) as a row of grey values, `scale` pixels a module. */
const draw = (digits: string, scale = 3): number[] => {
  const modules: boolean[] = [];
  const put = (widths: readonly number[], startDark: boolean) => {
    let dark = startDark;
    for (const width of widths) {
      for (let count = 0; count < width; count += 1) modules.push(dark);
      dark = !dark;
    }
  };
  const ean8 = digits.length === 8;
  const left = ean8 ? digits.slice(0, 4) : digits.slice(1, 7);
  const right = ean8 ? digits.slice(4) : digits.slice(7);
  const parity = ean8 ? '0000' : (FIRST_DIGIT_PARITY[Number(digits[0])] ?? '');
  put([1, 1, 1], true);
  [...left].forEach((digit, at) => {
    const pattern = [...(DIGITS[Number(digit)] ?? [])];
    put(parity[at] === '1' ? pattern.reverse() : pattern, false);
  });
  put([1, 1, 1, 1, 1], false);
  for (const digit of right) put(DIGITS[Number(digit)] ?? [], true);
  put([1, 1, 1], true);
  const quiet = new Array<number>(12 * scale).fill(250);
  return [
    ...quiet,
    ...modules.flatMap((dark) => new Array<number>(scale).fill(dark ? 20 : 240)),
    ...quiet,
  ];
};

describe('EAN barcodes', () => {
  it('checks check digits', () => {
    expect(checkDigitOk('9780306406157')).toBe(true);
    expect(checkDigitOk('9780306406158')).toBe(false);
    expect(checkDigitOk('0720642442517')).toBe(true);
  });

  it('reads an ISBN, a UPC-A and an EAN-8 from a row, either way round', () => {
    expect(decodeRow(draw('9780306406157'))).toBe('9780306406157');
    expect(decodeRow(draw('0720642442517').reverse())).toBe('0720642442517');
    expect(decodeRow(draw('96385074'))).toBe('96385074');
  });

  it('reads bars of uneven widths, as a camera sees them', () => {
    const row = draw('5099902894225', 4).map((value, at) => value + ((at * 7) % 13) - 6);
    expect(decodeRow(row)).toBe('5099902894225');
  });

  it('finds nothing in a row without a barcode', () => {
    expect(decodeRow(new Array<number>(300).fill(128))).toBeNull();
    expect(
      decodeRow([...new Array<number>(300)].map((_, at) => (at % 10 < 5 ? 0 : 255))),
    ).toBeNull();
  });

  it('reads a picture', () => {
    const row = draw('9780306406157');
    const width = row.length;
    const height = 20;
    const data = new Uint8ClampedArray(width * height * 4);
    for (let y = 0; y < height; y += 1) {
      row.forEach((value, x) => {
        const at = (y * width + x) * 4;
        data.set([value, value, value, 255], at);
      });
    }
    expect(decodeImage(data, width, height)).toBe('9780306406157');
  });
});
