/**
 * Sizes (M46.48): the screen gives every size, and a setting adds to it or takes from it. A size
 * is stored as a number; `zero` is the stored value that means "as the screen has it", and what
 * is shown is (stored − zero) × factor, e.g. "+10 %" for an interface size stored as 110.
 */
export type SizeLimits = {
  min: number;
  max: number;
  step: number;
  /** The stored value shown as 0, and what Reset goes back to. */
  zero?: number;
  /** Shown = (stored − zero) × factor, e.g. 0.1 for tenths. */
  factor?: number;
};

/** Decimals a step needs: 1 → 0, 0.1 → 1, 0.25 → 2. */
export const decimalsOf = (step: number) => {
  const text = String(Number(step.toPrecision(6)));
  return text.includes('.') ? text.length - text.indexOf('.') - 1 : 0;
};

/** A stored size as shown: what it adds to the screen's own, in shown units. */
export const shownSize = (stored: number, { zero = 0, factor = 1, step }: SizeLimits) =>
  Number(((stored - zero) * factor).toFixed(decimalsOf(step * factor)));

/** "+4", "−2", "0": a shown size with its sign. */
export const signedSize = (stored: number, limits: SizeLimits) => {
  const number = shownSize(stored, limits);
  const text = Math.abs(number).toFixed(decimalsOf(limits.step * (limits.factor ?? 1)));
  return number > 0 ? `+${text}` : number < 0 ? `−${text}` : text;
};

/**
 * Turns a typed number (in shown units, any minus sign, a decimal comma) into a stored value on
 * the step and within the limits; `null` when it isn't a number.
 */
export const parseSize = (text: string, { min, max, step, zero = 0, factor = 1 }: SizeLimits) => {
  const cleaned = text
    .trim()
    .replace(/[−‒–—]/g, '-')
    .replace(',', '.');
  if (cleaned === '' || cleaned === '-' || cleaned === '+') return null;
  const number = Number(cleaned);
  if (!Number.isFinite(number)) return null;
  const steps = Math.round((number / factor + zero - min) / step);
  const stored = Math.min(max, Math.max(min, min + steps * step));
  return Number(stored.toFixed(decimalsOf(step)));
};
