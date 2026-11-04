import { commands } from '@raygun/bindings';
import type { NovelReaderPrefs, TextBlock } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

export const NOVEL = 'novel';

export const DEFAULT_PREFS: NovelReaderPrefs = {
  fontSize: 18,
  lineSpacing: 16,
  font: 'serif',
  width: 'narrow',
  paged: false,
  ttsRate: 10,
  indent: false,
  theme: 'app',
  justify: false,
  keepScreenOn: true,
};

export const novelChapterQuery = (titleId: string, chapterId: string) => ({
  queryKey: [NOVEL, 'chapter', titleId, chapterId],
  queryFn: () => unwrapResult(commands.novelChapter(titleId, chapterId)),
  staleTime: 5 * 60_000,
});

export const useNovelChapter = (titleId: string, chapterId: string) =>
  useQuery({ ...novelChapterQuery(titleId, chapterId), enabled: Boolean(titleId && chapterId) });

/** Saves the character offset reached; the title page refetches when shown. */
export const saveNovelProgress = (chapterId: string, offset: number, total: number) =>
  unwrapResult(commands.novelSetProgress(chapterId, offset, total));

/** Characters a block adds to the chapter's text (what progress offsets count). */
const blockLength = (block: TextBlock) =>
  block.kind === 'paragraph' || block.kind === 'heading' ? block.text.length : 0;

/** Where each block starts in the chapter's text, and the text's length. */
export const textOffsets = (blocks: TextBlock[]) => {
  const starts: number[] = [];
  let total = 0;
  for (const block of blocks) {
    starts.push(total);
    total += blockLength(block);
  }
  return { starts, total: Math.max(total, 1) };
};

/** The block holding `offset`. */
export const blockAt = (starts: number[], offset: number) => {
  let index = 0;
  starts.forEach((start, candidate) => {
    if (start <= offset) index = candidate;
  });
  return index;
};

/** The paragraphs of a chapter from `first` on, as speech gets them (block index and text). */
export const speechParagraphs = (blocks: TextBlock[], first: number) =>
  blocks.flatMap((block, index) =>
    index >= first && (block.kind === 'paragraph' || block.kind === 'heading')
      ? [{ id: index, text: block.text }]
      : [],
  );
