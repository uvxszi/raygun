import type { PdfContentsEntry, TextBlock } from '@raygun/bindings';

import { currentEntry, outlineBlock, outlineTree } from './pdfContents';

const entry = (title: string, pageNumber: number, level: number): PdfContentsEntry => ({
  title,
  chapterId: 'c1',
  page: pageNumber - 1,
  pageNumber,
  level,
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
