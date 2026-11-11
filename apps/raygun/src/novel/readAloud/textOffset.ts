/** Where in a block's text (UTF-16, as Rust's reading aloud counts) a place on screen is. */

type CaretDocument = Document & {
  caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
  caretRangeFromPoint?: (x: number, y: number) => Range | null;
};

/** The text node and offset under a point on screen, when it's on text. */
const caretAt = (x: number, y: number): { node: Node; offset: number } | null => {
  const page = document as CaretDocument;
  if (typeof page.caretPositionFromPoint === 'function') {
    const position = page.caretPositionFromPoint(x, y);
    if (position) return { node: position.offsetNode, offset: position.offset };
  }
  if (typeof page.caretRangeFromPoint === 'function') {
    const range = page.caretRangeFromPoint(x, y);
    if (range) return { node: range.startContainer, offset: range.startOffset };
  }
  return null;
};

/** How far into `block`'s text a place (a node, and an offset in it) is. */
export const textOffsetOf = (block: Element, node: Node, offset: number): number | null => {
  if (!block.contains(node)) return null;
  try {
    const range = document.createRange();
    range.setStart(block, 0);
    range.setEnd(node, offset);
    return range.toString().length;
  } catch {
    return null;
  }
};

/** How far into `block`'s text the point (`x`, `y`) on screen is, when it's on its text. */
export const textOffsetAt = (block: HTMLElement, x: number, y: number): number | null => {
  // Text that can't be selected (word lookups off) gives no caret in some engines: it can be
  // while the caret is found.
  const previous = block.style.userSelect;
  block.style.userSelect = 'text';
  try {
    const caret = caretAt(x, y);
    return caret ? textOffsetOf(block, caret.node, caret.offset) : null;
  } finally {
    block.style.userSelect = previous;
  }
};
