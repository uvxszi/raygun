import type { PdfContentsEntry, TextBlock } from '@raygun/bindings';

/**
 * A PDF's outline as a tree, from its entries in order: each entry's parent is the nearest entry
 * before it at a shallower level. `depth` counts from the shallowest level the outline uses.
 */
export const outlineTree = (contents: PdfContentsEntry[]) => {
  const parents: (number | null)[] = [];
  const hasChildren = contents.map(() => false);
  const top = contents.reduce((lowest, entry) => Math.min(lowest, entry.level), Infinity);
  const stack: number[] = [];
  contents.forEach((entry, index) => {
    while (stack.length > 0 && contents[stack[stack.length - 1]!]!.level >= entry.level) {
      stack.pop();
    }
    const parent = stack.length > 0 ? stack[stack.length - 1]! : null;
    parents.push(parent);
    if (parent !== null) hasChildren[parent] = true;
    stack.push(index);
  });
  const depths = contents.map((entry) => entry.level - (Number.isFinite(top) ? top : 0));
  return { parents, hasChildren, depths };
};

/**
 * The entry being read at document page `pageNumber` (from 1): the last one leading to that page
 * or an earlier one, the deepest of those on one page. -1 before the first.
 */
export const currentEntry = (contents: PdfContentsEntry[], pageNumber: number) => {
  let found = -1;
  contents.forEach((entry, index) => {
    if (
      entry.pageNumber <= pageNumber &&
      (found < 0 || entry.pageNumber >= contents[found]!.pageNumber)
    ) {
      found = index;
    }
  });
  return found;
};

/** A heading's words for comparing: letters and digits only, in lower case. */
const words = (text: string) =>
  text
    .normalize('NFKD')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** A Roman numeral, in lower case. */
const ROMAN = '(?=[ivxlcdm])m{0,3}(?:cm|cd|d?c{0,3})(?:xc|xl|l?x{0,3})(?:ix|iv|v?i{0,3})';
/** A heading's number ("1.2", "Chapter 3", "IV.") before its words, as `words` leaves it. */
const NUMBERING = new RegExp(
  `^(?:(?:chapter|section|part|appendix|lesson|unit)\\s+)?(?:\\d+(?:\\s\\d+)*|${ROMAN})\\s+`,
);

/** Whether a block's text is the heading a bookmark names, with or without its number. */
const namesBookmark = (text: string, title: string) => {
  const block = words(text);
  if (!title || block.length > title.length + 24) return false;
  return block === title || block.replace(NUMBERING, '') === title || block.endsWith(` ${title}`);
};

/**
 * Where a bookmark leads in a PDF's text: the heading on its page (or the page after, where text
 * flows on) that names it, else the first block of its page.
 */
export const outlineBlock = (
  blocks: TextBlock[],
  pageStarts: number[],
  page: number,
  title: string,
) => {
  if (pageStarts.length === 0 || blocks.length === 0) return 0;
  const at = Math.max(0, Math.min(page, pageStarts.length - 1));
  const start = pageStarts[at] ?? 0;
  const end = Math.min(pageStarts[at + 2] ?? blocks.length, blocks.length);
  const wanted = words(title).replace(NUMBERING, '');
  for (let index = start; index < end; index += 1) {
    const block = blocks[index];
    if (
      block &&
      (block.kind === 'heading' || block.kind === 'paragraph') &&
      namesBookmark(block.text, wanted)
    ) {
      return index;
    }
  }
  return Math.min(start, blocks.length - 1);
};
