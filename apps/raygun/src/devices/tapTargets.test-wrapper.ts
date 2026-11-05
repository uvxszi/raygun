/**
 * Checks for the device screens' tap sizes (`tapTargets.ts`): the layout isn't measured in tests,
 * so what's checked is that every target carries a size of at least 44 px.
 */

/** A minimum height or a size of 11 (44 px) or more. */
export const ROOMY = /(?:^|\s)(?:min-h|size)-(?:1[1-9]|[2-9]\d)(?:\s|$)/;

/** Everything in `root` that can be tapped. */
export const tapTargets = (root: HTMLElement) => [
  ...root.querySelectorAll<HTMLElement>('button, a[href], [role="switch"]'),
];

/** What a target is called, to say which one is too small. */
export const labelOf = (target: HTMLElement) =>
  target.getAttribute('aria-label') ?? target.textContent ?? target.tagName;
