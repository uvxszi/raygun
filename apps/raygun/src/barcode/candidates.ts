import { checkDigitOk } from './ean';

/**
 * Codes read while scanning (v2.0.1). A camera can misread a barcode once in a while (a digit
 * off, a bar lost to glare), so the sheet keeps reading for a moment after the first code, counts
 * what each frame read, throws out codes whose check digit fails, and offers the codes read most.
 */

/** How long reading goes on after the first valid code. */
export const COLLECT_MS = 2000;
/** A code read this many times ends the reading at once. */
export const ENOUGH_READS = 3;
/** How many codes are offered to pick from. */
export const SHOWN = 3;

/** A UPC-E code (number system, six digits, check digit) written out as its UPC-A. */
export const expandUpcE = (code: string): string | null => {
  if (!/^[01]\d{7}$/.test(code)) return null;
  const system = code[0] ?? '0';
  const [d1, d2, d3, d4, d5, d6] = code.slice(1, 7).split('');
  const check = code[7] ?? '';
  let body: string;
  switch (d6) {
    case '0':
    case '1':
    case '2':
      body = `${d1}${d2}${d6}0000${d3}${d4}${d5}`;
      break;
    case '3':
      body = `${d1}${d2}${d3}00000${d4}${d5}`;
      break;
    case '4':
      body = `${d1}${d2}${d3}${d4}00000${d5}`;
      break;
    default:
      body = `${d1}${d2}${d3}${d4}${d5}0000${d6}`;
  }
  return `${system}${body}${check}`;
};

/**
 * The code to look up when `raw` is a valid product barcode, else null: EAN-13 (ISBNs among
 * them), UPC-A, EAN-8 and UPC-E with their check digits right. UPC-A codes are written as their
 * EAN-13 (a leading 0), so the same barcode read either way counts once; UPC-E as its UPC-A.
 * `format` is what the browser's reader said it read, when it said.
 */
export const validCode = (raw: string, format?: string): string | null => {
  const digits = raw.replace(/[\s-]/g, '');
  if (!/^\d+$/.test(digits)) return null;
  if (
    format === 'upc_e' ||
    (format === undefined && digits.length === 8 && !checkDigitOk(digits))
  ) {
    const expanded = expandUpcE(digits);
    return expanded && checkDigitOk(expanded) ? `0${expanded}` : null;
  }
  switch (digits.length) {
    case 13:
      return checkDigitOk(digits) ? digits : null;
    case 12:
      return checkDigitOk(digits) ? `0${digits}` : null;
    case 8:
      return checkDigitOk(digits) ? digits : null;
    default:
      return null;
  }
};

/** A code read while scanning, with how often. */
export type Candidate = { code: string; reads: number };

/** The reads so far: each code's count and the order codes first came. */
export type Tally = Map<string, number>;

/** Counts one read; answers the code's count now (0 when the read isn't a valid code). */
export const countRead = (tally: Tally, raw: string, format?: string): number => {
  const code = validCode(raw, format);
  if (!code) return 0;
  const reads = (tally.get(code) ?? 0) + 1;
  tally.set(code, reads);
  return reads;
};

/** The codes read most, most first (ties: the one read first), at most `SHOWN`. */
export const rankCandidates = (tally: Tally, limit = SHOWN): Candidate[] =>
  [...tally.entries()]
    .map(([code, reads], order) => ({ code, reads, order }))
    .sort((left, right) => right.reads - left.reads || left.order - right.order)
    .slice(0, limit)
    .map(({ code, reads }) => ({ code, reads }));

/**
 * Whether the first code clearly won: the only one read, or read at least `ENOUGH_READS` times
 * and at least twice as often as the next.
 */
export const isDominant = (ranked: Candidate[]): boolean => {
  const [first, second] = ranked;
  if (!first) return false;
  if (!second) return true;
  return first.reads >= ENOUGH_READS && first.reads >= second.reads * 2;
};
