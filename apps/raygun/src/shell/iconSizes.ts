/**
 * Icons drawn at a size in px (lucide's `size`, and Raygun's own icons marked `lucide`) follow the
 * interface the way the buttons around them do, and land on whole device pixels so their strokes
 * stay crisp.
 *
 * - `--icon-unit` is what one px of an icon's size is (styles.css): 1px in the desktop app (the
 *   window zooms), Interface size's share of a px on the web's desktop layout, and 1/16 rem on
 *   phones, where the screen and Interface size set the root font size every button follows
 *   (settings/useApplySize).
 * - `--device-px` is one device pixel in CSS px (`1px / devicePixelRatio`, which on desktop
 *   includes the window's zoom); sizes round to it with CSS `round()` where the engine has it.
 *
 * Sizes that are whole numbers get a rule each (every engine), others follow typed `attr()` where
 * the engine reads it. The rules sit in the base layer, so an icon sized by a class keeps that.
 */

/** The whole-number sizes given rules of their own. */
export const ICON_SIZES: readonly number[] = Array.from({ length: 128 - 6 + 1 }, (_, i) => i + 6);

/** The CSS length of an icon `size` px wide: scaled, then rounded to device pixels. */
export const iconLength = (size: number | string) => {
  const scaled = `calc(${size} * var(--icon-unit, 1px))`;
  return { scaled, rounded: `round(nearest, ${scaled}, var(--device-px, 1px))` };
};

const sides = (value: string) => `width: ${value}; height: ${value};`;

/** Both declarations: the plain one stays where `round()` is unknown and the line is dropped. */
const sized = (size: number | string) => {
  const { scaled, rounded } = iconLength(size);
  return `${sides(scaled)} ${sides(rounded)}`;
};

/** The stylesheet, in the base layer. */
export const iconSizeCss = (sizes: readonly number[] = ICON_SIZES) =>
  [
    '@layer base {',
    // Never squeezed by a row, never wider or taller than a button that caps its own size
    // (`min(3rem, 15cqw)`): the drawing keeps its shape (the viewBox) either way.
    '  svg.lucide { flex-shrink: 0; max-width: 100%; max-height: 100%; }',
    '  @supports (width: calc(attr(width type(<number>), 0) * 1px)) {',
    `    svg.lucide[width] { ${sized('attr(width type(<number>), 24)')} }`,
    '  }',
    ...sizes.map((size) => `  svg.lucide[width="${size}"] { ${sized(size)} }`),
    '}',
  ].join('\n');

const STYLE_ID = 'raygun-icon-sizes';

/** Adds the stylesheet once. */
export const installIconSizes = (doc: Document = document) => {
  if (doc.getElementById(STYLE_ID)) return;
  const style = doc.createElement('style');
  style.id = STYLE_ID;
  style.textContent = iconSizeCss();
  doc.head.appendChild(style);
};

/** Keeps `--device-px` at one device pixel as the screen or the zoom changes. */
export const followDevicePixels = (root: HTMLElement = document.documentElement) => {
  let query: MediaQueryList | undefined;
  const update = () => {
    const ratio = window.devicePixelRatio || 1;
    root.style.setProperty('--device-px', `${1 / ratio}px`);
    query?.removeEventListener('change', update);
    query = window.matchMedia?.(`(resolution: ${ratio}dppx)`);
    query?.addEventListener('change', update);
  };
  update();
  return () => query?.removeEventListener('change', update);
};
