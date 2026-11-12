import type { MangaChapterInfo, PdfContentsEntry, TextBlock } from '@raygun/bindings';

import {
  bookContents,
  currentEntry,
  currentRow,
  hasContents,
  levelTree,
  outlineBlock,
  outlineTree,
  placeOf,
} from './pdfContents';
import type { ContentsRow } from './pdfContents';

const entry = (title: string, pageNumber: number, level: number): PdfContentsEntry => ({
  title,
  chapterId: 'c1',
  page: pageNumber - 1,
  pageNumber,
  level,
});

const chapter = (id: string, name: string, number: number | null): MangaChapterInfo => ({
  id,
  name,
  number,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

/** A bookmark of chapter `chapterId`, `page` pages into it. */
const mark = (
  title: string,
  chapterId: string,
  page: number,
  pageNumber: number,
  level: number,
): PdfContentsEntry => ({ title, chapterId, page, pageNumber, level });

/** Rows as their names, set in by their depth. */
const outline = (rows: ContentsRow[]) => {
  const { depths } = levelTree(rows.map((row) => row.level));
  return rows.map(
    (row, index) =>
      `${'  '.repeat(depths[index] ?? 0)}${row.kind === 'chapter' ? `[${row.chapter.name}]` : row.entry.title}`,
  );
};

describe('A PDF book’s contents with its chapters', () => {
  it('lists a book’s outline as it is, with the chapters it leads into none of', () => {
    // An imported book: front matter before the first bookmark, then a chapter per part.
    const marks = [
      mark('Part I', 'p1', 0, 5, 0),
      mark('Lighthouses', 'p1', 0, 5, 1),
      mark('Storms', 'p1', 6, 11, 1),
      mark('Part II', 'p2', 0, 20, 0),
    ];
    const chapters = [
      chapter('p2', 'Part II', 3),
      chapter('front', 'Front matter', 1),
      chapter('p1', 'Part I', 2),
    ];
    const rows = bookContents(marks, chapters);
    expect(outline(rows)).toEqual([
      '[Front matter]',
      'Part I',
      '  Lighthouses',
      '  Storms',
      'Part II',
    ]);
    // Reading the front matter marks its row; reading a part, its bookmark.
    expect(currentRow(rows, marks, 'front', 2)).toBe(0);
    expect(currentRow(rows, marks, 'p1', 12)).toBe(3);
    // A chapter leads to its start, its heading named as it is.
    expect(placeOf(rows[0]!)).toEqual({ chapterId: 'front', page: 0, title: 'Front matter' });
    expect(placeOf(rows[3]!)).toBe(marks[2]);
  });

  it('lists the parts of a document without bookmarks', () => {
    const parts = [chapter('a', 'Pages 1–40', 1), chapter('b', 'Pages 41–80', 2)];
    const rows = bookContents([], parts);
    expect(outline(rows)).toEqual(['[Pages 1–40]', '[Pages 41–80]']);
    expect(currentRow(rows, [], 'b', 50)).toBe(1);
  });

  it('lists every chapter of a source, the bookmarks under their own', () => {
    // A source's chapter carries the bookmarks of its own pages only.
    const marks = [mark('Cover', 'c2', 0, 1, 0), mark('Lesson 1', 'c2', 3, 4, 0)];
    const chapters = [chapter('c1', 'Volume 1', null), chapter('c2', 'Volume 2', null)];
    const rows = bookContents(marks, chapters);
    expect(outline(rows)).toEqual(['[Volume 1]', '[Volume 2]', '  Cover', '  Lesson 1']);
    expect(currentRow(rows, marks, 'c2', 6)).toBe(3);
    expect(currentRow(rows, marks, 'c1', 6)).toBe(0);
  });

  it('lists the outline alone until the chapters are known', () => {
    const marks = [mark('One', 'c1', 0, 1, 0), mark('Two', 'c2', 0, 9, 0)];
    expect(outline(bookContents(marks, []))).toEqual(['One', 'Two']);
    expect(currentRow(bookContents(marks, []), marks, 'c2', 9)).toBe(1);
  });

  it('has contents to list with bookmarks or other chapters', () => {
    const alone = { previousChapterId: null, nextChapterId: null };
    expect(hasContents([], alone)).toBe(false);
    expect(hasContents([], { ...alone, nextChapterId: 'c2' })).toBe(true);
    expect(hasContents([mark('One', 'c1', 0, 1, 0)], alone)).toBe(true);
  });
});

describe('PDF contents', () => {
  const contents = [
    entry('A Book', 1, 1),
    entry('Preface', 1, 2),
    entry('Chapter 1', 5, 2),
    entry('1.1 Sets', 5, 3),
    entry('1.2 Maps', 7, 3),
    entry('Chapter 2', 9, 2),
  ];

  it('makes a tree of the outline, depths counted from its top', () => {
    const tree = outlineTree(contents);
    expect(tree.parents).toEqual([null, 0, 0, 2, 2, 0]);
    expect(tree.hasChildren).toEqual([true, false, true, false, false, false]);
    expect(tree.depths).toEqual([0, 1, 1, 2, 2, 1]);
    expect(outlineTree([])).toEqual({ parents: [], hasChildren: [], depths: [] });
  });

  it('marks the deepest part begun at or before the page being read', () => {
    expect(currentEntry(contents, 5)).toBe(3);
    expect(currentEntry(contents, 6)).toBe(3);
    expect(currentEntry(contents, 8)).toBe(4);
    expect(currentEntry(contents, 30)).toBe(5);
    expect(currentEntry([entry('Late', 4, 0)], 2)).toBe(-1);
  });

  it('finds the heading a bookmark names in the text, else its page’s start', () => {
    const blocks: TextBlock[] = [
      { kind: 'heading', text: 'Chapter 1' },
      { kind: 'paragraph', text: 'Text running on from the page before.' },
      { kind: 'heading', text: '1.2 Running Python' },
      { kind: 'paragraph', text: 'Python is run in a terminal.' },
      { kind: 'heading', text: 'Exercises' },
    ];
    const pageStarts = [0, 1, 4];
    expect(outlineBlock(blocks, pageStarts, 1, 'Running Python')).toBe(2);
    expect(outlineBlock(blocks, pageStarts, 1, '1.2 Running Python')).toBe(2);
    expect(outlineBlock(blocks, pageStarts, 0, 'The way of the program')).toBe(0);
    // A heading that flowed onto the next page is still found.
    expect(outlineBlock(blocks, pageStarts, 1, 'Exercises')).toBe(4);
    // Not on its page: the page's start; past the end: the last block.
    expect(outlineBlock(blocks, pageStarts, 2, 'Glossary')).toBe(4);
    expect(outlineBlock(blocks, [0, 5], 1, 'Glossary')).toBe(4);
    expect(outlineBlock([], [], 0, 'Anything')).toBe(0);
  });
});
