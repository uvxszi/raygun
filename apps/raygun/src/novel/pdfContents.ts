import type { MangaChapterInfo, PdfContentsEntry, TextBlock } from '@raygun/bindings';

/**
 * Rows at `levels` as a tree, in order: each row's parent is the nearest row before it at a
 * shallower level. `depth` counts from the shallowest level used.
 */
export const levelTree = (levels: number[]) => {
  const parents: (number | null)[] = [];
  const hasChildren = levels.map(() => false);
  const top = levels.reduce((lowest, level) => Math.min(lowest, level), Infinity);
  const stack: number[] = [];
  levels.forEach((level, index) => {
    while (stack.length > 0 && levels[stack[stack.length - 1]!]! >= level) stack.pop();
    const parent = stack.length > 0 ? stack[stack.length - 1]! : null;
    parents.push(parent);
    if (parent !== null) hasChildren[parent] = true;
    stack.push(index);
  });
  const depths = levels.map((level) => level - (Number.isFinite(top) ? top : 0));
  return { parents, hasChildren, depths };
};

/** A PDF's outline as a tree, from its entries in order (`levelTree`). */
export const outlineTree = (contents: PdfContentsEntry[]) =>
  levelTree(contents.map((entry) => entry.level));

/** A row of a PDF book's contents: one of its bookmarks, or one of its chapters. */
export type ContentsRow =
  | { kind: 'bookmark'; entry: PdfContentsEntry; level: number }
  | { kind: 'chapter'; chapter: MangaChapterInfo; level: number };

/** Where a row of the contents leads: a page of a chapter, and the heading's name there. */
export type ContentsPlace = Pick<PdfContentsEntry, 'chapterId' | 'page' | 'title'>;

/** A bookmark's page, or a chapter's start (its heading there, when its text names it). */
export const placeOf = (row: ContentsRow): ContentsPlace =>
  row.kind === 'bookmark'
    ? row.entry
    : { chapterId: row.chapter.id, page: 0, title: row.chapter.name };

/** A book's chapters in reading order: by number when every one has one, else as listed. */
export const readingOrder = (chapters: MangaChapterInfo[]) =>
  chapters.every((chapter) => chapter.number !== null)
    ? [...chapters].sort((a, b) => (a.number ?? 0) - (b.number ?? 0))
    : chapters;

/**
 * A PDF book's contents with its chapters, so one list leads everywhere in the book. A book's
 * outline (its bookmarks lead into several of its chapters, or into its only one) is listed as
 * it is, with the chapters it leads into none of (a book's front matter, the parts of a document
 * without bookmarks) in their places among the bookmarks. A source's chapter carries bookmarks
 * of its own pages only: then every chapter is listed, with those bookmarks under their chapter.
 * Without the chapters (not loaded yet), the outline alone.
 */
export const bookContents = (
  contents: PdfContentsEntry[],
  chapters: MangaChapterInfo[],
): ContentsRow[] => {
  const outline = (entry: PdfContentsEntry): ContentsRow => ({
    kind: 'bookmark',
    entry,
    level: entry.level,
  });
  if (chapters.length === 0) return contents.map(outline);
  // Chapters' own rows are at the outline's top level.
  const shallowest = contents.reduce((lowest, entry) => Math.min(lowest, entry.level), Infinity);
  const top = Number.isFinite(shallowest) ? shallowest : 0;
  const byChapter = new Map<string, PdfContentsEntry[]>();
  for (const entry of contents) {
    const marks = byChapter.get(entry.chapterId);
    if (marks) marks.push(entry);
    else byChapter.set(entry.chapterId, [entry]);
  }
  const ordered = readingOrder(chapters);
  const ownChapter =
    ordered.length > 1 &&
    byChapter.size === 1 &&
    ordered.some((chapter) => byChapter.has(chapter.id));
  const rows: ContentsRow[] = [];
  for (const chapter of ordered) {
    const marks = byChapter.get(chapter.id) ?? [];
    byChapter.delete(chapter.id);
    if (ownChapter || marks.length === 0) rows.push({ kind: 'chapter', chapter, level: top });
    for (const entry of marks) {
      rows.push(ownChapter ? { kind: 'bookmark', entry, level: entry.level + 1 } : outline(entry));
    }
  }
  // Bookmarks into chapters not listed keep their place in the outline, after the rest.
  for (const marks of byChapter.values()) rows.push(...marks.map(outline));
  return rows;
};

/**
 * The row being read: the bookmark `currentEntry` finds when it's in the chapter being read, else
 * that chapter's own row, else (chapters not loaded) the bookmark found. -1 for none.
 */
export const currentRow = (
  rows: ContentsRow[],
  contents: PdfContentsEntry[],
  chapterId: string,
  pageNumber: number,
) => {
  const found = contents[currentEntry(contents, pageNumber)];
  const bookmark = rows.findIndex((row) => row.kind === 'bookmark' && row.entry === found);
  if (found && found.chapterId === chapterId && bookmark >= 0) return bookmark;
  const chapter = rows.findIndex((row) => row.kind === 'chapter' && row.chapter.id === chapterId);
  return chapter >= 0 ? chapter : bookmark;
};

/** Whether a PDF chapter has contents to list: bookmarks, or other chapters to go to. */
export const hasContents = (
  contents: PdfContentsEntry[],
  chapter: { previousChapterId: string | null; nextChapterId: string | null },
) => contents.length > 0 || chapter.previousChapterId !== null || chapter.nextChapterId !== null;

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
