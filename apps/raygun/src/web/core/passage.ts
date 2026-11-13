// A book source's `passage` answer as the reader's text blocks, in the browser (the app reads it
// with raygun-ext-host's `novel` module). HTML is parsed into an inert document and walked:
// headings, paragraphs, code, pictures and rules; scripts, styles and anything else that would
// run or reach out are never put on the page, only the text and picture addresses are kept.
import type { TextBlock_Serialize } from '@raygun/bindings';

const SKIPPED = new Set([
  'SCRIPT',
  'STYLE',
  'NOSCRIPT',
  'TEMPLATE',
  'HEAD',
  'IFRAME',
  'OBJECT',
  'SVG',
  'MATH',
  'BUTTON',
  'FORM',
  'INPUT',
  'SELECT',
  'TEXTAREA',
]);

/** Elements that start a block of their own; anything else flows into the paragraph around it. */
const BLOCKS = new Set([
  'ADDRESS',
  'ARTICLE',
  'ASIDE',
  'BLOCKQUOTE',
  'BODY',
  'CAPTION',
  'CENTER',
  'DD',
  'DIV',
  'DL',
  'DT',
  'FIGCAPTION',
  'FIGURE',
  'FOOTER',
  'HEADER',
  'LI',
  'MAIN',
  'NAV',
  'OL',
  'P',
  'SECTION',
  'TABLE',
  'TBODY',
  'TD',
  'TFOOT',
  'TH',
  'THEAD',
  'TR',
  'UL',
]);

const HEADINGS = new Set(['H1', 'H2', 'H3', 'H4', 'H5', 'H6']);

const webAddress = (value: string | null, base: string | null): string | null => {
  if (!value) return null;
  try {
    const address = new URL(value, base ?? undefined);
    return address.protocol === 'https:' || address.protocol === 'http:' ? address.href : null;
  } catch {
    return null;
  }
};

const tidy = (text: string) => text.replace(/\s+/g, ' ').trim();

/** Blocks from HTML, pictures' addresses made whole against `baseUrl`. */
export const blocksFromHtml = (html: string, baseUrl: string | null): TextBlock_Serialize[] => {
  const base = webAddress(baseUrl, null);
  const document = new DOMParser().parseFromString(html, 'text/html');
  const blocks: TextBlock_Serialize[] = [];
  let pending = '';
  const flush = () => {
    const text = tidy(pending);
    pending = '';
    if (text) blocks.push({ kind: 'paragraph', text });
  };
  const walk = (node: Node) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === 3) {
        pending += child.textContent ?? '';
        continue;
      }
      if (child.nodeType !== 1) continue;
      const element = child as Element;
      const tag = element.tagName.toUpperCase();
      if (SKIPPED.has(tag)) continue;
      if (HEADINGS.has(tag)) {
        flush();
        const text = tidy(element.textContent ?? '');
        if (text) blocks.push({ kind: 'heading', text });
      } else if (tag === 'PRE') {
        flush();
        const text = (element.textContent ?? '').replace(/^\n+|\s+$/g, '');
        if (text) blocks.push({ kind: 'code', text });
      } else if (tag === 'HR') {
        flush();
        blocks.push({ kind: 'separator' });
      } else if (tag === 'BR') {
        flush();
      } else if (tag === 'IMG') {
        const url = webAddress(element.getAttribute('src') ?? element.getAttribute('data-src'), base);
        if (url) {
          flush();
          blocks.push({ kind: 'image', url });
        }
      } else if (BLOCKS.has(tag)) {
        flush();
        walk(element);
        flush();
      } else {
        walk(element);
      }
    }
  };
  walk(document.body);
  flush();
  return blocks;
};

/** Blocks from plain text: a paragraph per line or blank-line-separated run. */
export const blocksFromText = (text: string): TextBlock_Serialize[] =>
  text
    .split(/\n\s*\n|\r?\n/)
    .map(tidy)
    .filter(Boolean)
    .map((line) => ({ kind: 'paragraph', text: line }));

/** A `passage` answer's blocks; a PDF chapter (`pdf`) needs the app's renderer. */
export const passageBlocks = (answer: unknown): TextBlock_Serialize[] => {
  const value = (answer ?? {}) as Record<string, unknown>;
  if (value.pdf) {
    throw new Error(
      'This chapter is a PDF document, which only the Raygun app can show. Link one in Devices to read it there.',
    );
  }
  if (typeof value.html === 'string') {
    return blocksFromHtml(value.html, typeof value.baseUrl === 'string' ? value.baseUrl : null);
  }
  if (typeof value.text === 'string') return blocksFromText(value.text);
  return [];
};
