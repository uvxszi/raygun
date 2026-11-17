/**
 * Reading EAN-13, UPC-A and EAN-8 barcodes (a CD's, a record's, a book's ISBN) from pictures
 * where the WebView has no barcode reader of its own: rows of the picture are made black and
 * white, their bars measured, and the digits matched against the symbology's patterns, with the
 * check digit checked.
 */

/** The L-code digit patterns in modules (space, bar, space, bar); R codes are the same widths
 * starting with a bar, G codes the same widths reversed. */
export const DIGITS = [
  [3, 2, 1, 1],
  [2, 2, 2, 1],
  [2, 1, 2, 2],
  [1, 4, 1, 1],
  [1, 1, 3, 2],
  [1, 2, 3, 1],
  [1, 1, 1, 4],
  [1, 3, 1, 2],
  [1, 2, 1, 3],
  [3, 1, 1, 2],
] as const;

/** The first digit of an EAN-13, by which of the left six digits are G codes (1) or L codes. */
export const FIRST_DIGIT_PARITY = [
  '000000',
  '001011',
  '001101',
  '001110',
  '010011',
  '011001',
  '011100',
  '010101',
  '010110',
  '011010',
] as const;

/** A digit's pattern is accepted when its widths are this close (in modules, summed). */
const MAX_ERROR = 1.7;

/** Whether the digits pass the GS1 check (weights 3 and 1 from the right). */
export const checkDigitOk = (digits: string) => {
  let sum = 0;
  for (let at = 0; at < digits.length; at += 1) {
    const digit = Number(digits[digits.length - 1 - at]);
    sum += at % 2 === 1 ? digit * 3 : digit;
  }
  return sum % 10 === 0;
};

/** Widths of the runs of dark and light pixels in a row, starting with the first dark one. */
export const runsOf = (row: ArrayLike<number>): number[] => {
  let low = 255;
  let high = 0;
  for (let at = 0; at < row.length; at += 1) {
    const value = row[at] ?? 0;
    if (value < low) low = value;
    if (value > high) high = value;
  }
  if (high - low < 40) return [];
  const middle = (low + high) / 2;
  const runs: number[] = [];
  let dark = false;
  let length = 0;
  // The light before the first bar counts as a run too: it's the quiet zone.
  for (let at = 0; at < row.length; at += 1) {
    const isDark = (row[at] ?? 255) < middle;
    if (isDark === dark) {
      length += 1;
    } else {
      runs.push(length);
      dark = isDark;
      length = 1;
    }
  }
  runs.push(length);
  return runs;
};

/** The best digit for four runs: its value, whether it's a G code, and how far off it is. */
const matchDigit = (widths: number[], codes: 'left' | 'right') => {
  const total = widths.reduce((sum, width) => sum + width, 0);
  const modules = widths.map((width) => (width * 7) / total);
  let best = { digit: -1, g: false, error: Infinity };
  DIGITS.forEach((pattern, digit) => {
    const forms = codes === 'left' ? [false, true] : [false];
    for (const g of forms) {
      const wanted = g ? [...pattern].reverse() : pattern;
      const error = modules.reduce(
        (sum, value, at) => sum + Math.abs(value - (wanted[at] ?? 0)),
        0,
      );
      if (error < best.error) best = { digit, g, error };
    }
  });
  return best;
};

const near = (width: number, module: number, count: number) =>
  Math.abs(width - module * count) <= Math.max(1.5, module * count * 0.6);

/** Digits read from runs starting at a bar (`at`, an odd index of `runs`), or null. */
const decodeAt = (runs: number[], at: number, length: 13 | 8): string | null => {
  const half = length === 13 ? 6 : 4;
  const needed = 3 + half * 4 + 5 + half * 4 + 3;
  if (at + needed > runs.length) return null;
  const guard = runs.slice(at, at + 3);
  const module = guard.reduce((sum, width) => sum + width, 0) / 3;
  if (!guard.every((width) => near(width, module, 1))) return null;
  // A quiet zone before the start: light for a few modules at least.
  if ((runs[at - 1] ?? 0) < module * 3) return null;
  let digits = '';
  let parity = '';
  let next = at + 3;
  for (let count = 0; count < half; count += 1) {
    const found = matchDigit(runs.slice(next, next + 4), 'left');
    if (found.error > MAX_ERROR) return null;
    digits += String(found.digit);
    parity += found.g ? '1' : '0';
    next += 4;
  }
  const middle = runs.slice(next, next + 5);
  if (!middle.every((width) => near(width, module, 1))) return null;
  next += 5;
  for (let count = 0; count < half; count += 1) {
    const found = matchDigit(runs.slice(next, next + 4), 'right');
    if (found.error > MAX_ERROR) return null;
    digits += String(found.digit);
    next += 4;
  }
  const end = runs.slice(next, next + 3);
  if (!end.every((width) => near(width, module, 1))) return null;
  if (length === 8) {
    if (parity !== '0000') return null;
    return checkDigitOk(digits) ? digits : null;
  }
  const first = FIRST_DIGIT_PARITY.indexOf(parity as (typeof FIRST_DIGIT_PARITY)[number]);
  if (first < 0) return null;
  const all = `${first}${digits}`;
  return checkDigitOk(all) ? all : null;
};

/** A barcode in one row of grey values (0 dark to 255 light), read either way round. */
export const decodeRow = (row: ArrayLike<number>): string | null => {
  const values = Array.from(row);
  for (const order of [values, [...values].reverse()]) {
    const runs = runsOf(order);
    // Odd indexes are bars (index 0 is the light before the first bar).
    for (let at = 1; at < runs.length; at += 2) {
      const found = decodeAt(runs, at, 13) ?? decodeAt(runs, at, 8);
      if (found) return found;
    }
  }
  return null;
};

/** Rows read, from the middle out: barcodes are usually near the middle of what's aimed at. */
const ROW_SHARES = [0.5, 0.45, 0.55, 0.4, 0.6, 0.35, 0.65, 0.3, 0.7, 0.25, 0.75, 0.2, 0.8];

/** A barcode in a picture's pixels (RGBA, as a canvas gives them), or null. */
export const decodeImage = (data: ArrayLike<number>, width: number, height: number) => {
  for (const share of ROW_SHARES) {
    const y = Math.min(height - 1, Math.round(height * share));
    const row = new Array<number>(width);
    for (let x = 0; x < width; x += 1) {
      const at = (y * width + x) * 4;
      row[x] = 0.299 * (data[at] ?? 0) + 0.587 * (data[at + 1] ?? 0) + 0.114 * (data[at + 2] ?? 0);
    }
    const found = decodeRow(row);
    if (found) return found;
  }
  return null;
};
