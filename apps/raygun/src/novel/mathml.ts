/**
 * Formulas in book text arrive as MathML Rust already rebuilt from an allowlist; it's checked
 * again here before it's drawn, against the same allowlist: MathML elements and their attributes
 * only, nothing that runs, links or loads.
 */
const ELEMENTS = new Set([
  'math',
  'semantics',
  'annotation',
  'mrow',
  'mi',
  'mn',
  'mo',
  'ms',
  'mtext',
  'mspace',
  'mfrac',
  'msqrt',
  'mroot',
  'msub',
  'msup',
  'msubsup',
  'munder',
  'mover',
  'munderover',
  'mmultiscripts',
  'mprescripts',
  'none',
  'mtable',
  'mtr',
  'mtd',
  'mstyle',
  'mpadded',
  'mphantom',
  'merror',
  'menclose',
]);

const ATTRIBUTES = new Set([
  'xmlns',
  'display',
  'displaystyle',
  'scriptlevel',
  'mathvariant',
  'mathsize',
  'mathcolor',
  'mathbackground',
  'dir',
  'form',
  'fence',
  'separator',
  'stretchy',
  'symmetric',
  'largeop',
  'movablelimits',
  'accent',
  'accentunder',
  'lspace',
  'rspace',
  'minsize',
  'maxsize',
  'linethickness',
  'width',
  'height',
  'depth',
  'voffset',
  'notation',
  'columnalign',
  'rowalign',
  'columnspacing',
  'rowspacing',
  'columnlines',
  'rowlines',
  'columnspan',
  'rowspan',
  'frame',
  'framespacing',
  'encoding',
  'class',
  'style',
]);

/** A value that can't load or run anything (no `url(`, no script words, no markup). */
const SAFE_VALUE = /^[\w\s.,#%+\-:;()/]*$/;

const clean = (element: Element) => {
  for (const child of [...element.children]) {
    if (!ELEMENTS.has(child.localName)) {
      child.remove();
      continue;
    }
    for (const attribute of [...child.attributes]) {
      const value = attribute.value.toLowerCase();
      const safe =
        ATTRIBUTES.has(attribute.localName) &&
        SAFE_VALUE.test(attribute.value) &&
        !value.includes('url') &&
        !value.includes('expression') &&
        !value.includes('javascript');
      if (!safe) child.removeAttribute(attribute.name);
    }
    clean(child);
  }
};

/** The `<math>` element of `markup`, cleaned; `null` when there's none. */
export const safeMath = (markup: string): Element | null => {
  if (typeof DOMParser === 'undefined') return null;
  const parsed = new DOMParser().parseFromString(`<body>${markup}</body>`, 'text/html');
  const math = parsed.body.querySelector('math');
  if (!math) return null;
  const holder = parsed.createElement('div');
  holder.append(math);
  clean(holder);
  const kept = holder.querySelector('math');
  return kept ? document.importNode(kept, true) : null;
};
