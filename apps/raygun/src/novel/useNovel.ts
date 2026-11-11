import { commands } from '@raygun/bindings';
import type { NovelReaderPrefs, TextBlock } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { spokenText } from './mathml';

export const NOVEL = 'novel';

export const DEFAULT_PREFS: NovelReaderPrefs = {
  textSize: 0,
  lineSpacing: 16,
  font: 'serif',
  width: 'narrow',
  paged: false,
  ttsRate: 10,
  indent: 0,
  theme: 'app',
  justify: false,
  keepScreenOn: true,
};

/** Matches `NOVEL_TEXT_SIZES` in raygun-model: px more or less than the screen's own (M46.48). */
export const TEXT_SIZE = { min: -6, max: 12, step: 1 } as const;
/** Line spacing in tenths, 1.2 to 2.2. */
export const LINE_SPACING = { min: 12, max: 22, step: 1 } as const;
/** Matches `NOVEL_INDENTS` in raygun-model: the paragraph indent in tenths of an em, none to 4. */
export const INDENT = { min: 0, max: 40, step: 1 } as const;

/** A paragraph indent (tenths of an em) as CSS, or nothing for none. */
export const indentStyle = (tenths: number | null | undefined) =>
  tenths && tenths > 0 ? { textIndent: `${tenths / 10}em` } : {};

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
  block.kind === 'paragraph' ||
  block.kind === 'heading' ||
  block.kind === 'math' ||
  block.kind === 'code'
    ? block.text.length
    : 0;

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

/**
 * The paragraphs of a chapter from `first` on, as speech gets them (block index and text):
 * formulas in words.
 */
export const speechParagraphs = (blocks: TextBlock[], first: number) =>
  blocks.flatMap((block, index) => {
    const text = index >= first ? spokenText(block) : null;
    return text ? [{ id: index, text }] : [];
  });

/** A PDF chapter's text, read from its pages (with where each page starts). */
export const pdfTextQuery = (titleId: string, chapterId: string) => ({
  queryKey: [NOVEL, 'pdfText', titleId, chapterId],
  queryFn: () => unwrapResult(commands.novelPdfText(titleId, chapterId)),
  staleTime: 30 * 60_000,
});

export const usePdfText = (titleId: string, chapterId: string, enabled: boolean) =>
  useQuery({
    ...pdfTextQuery(titleId, chapterId),
    enabled: enabled && Boolean(titleId && chapterId),
  });

/** How a PDF book is read: its pages, or its text. */
export type PdfView = 'pages' | 'text';

/** The view a PDF book was last read in, kept on this device per title. */
export const usePdfView = (titleId: string) => {
  const key = `raygun.pdfView.${titleId}`;
  const [view, setView] = useState<PdfView>(() => {
    try {
      return localStorage.getItem(key) === 'text' ? 'text' : 'pages';
    } catch {
      return 'pages';
    }
  });
  const choose = useCallback(
    (next: PdfView) => {
      setView(next);
      try {
        localStorage.setItem(key, next);
      } catch {
        // Not kept: the view still changes now.
      }
    },
    [key],
  );
  return [view, choose] as const;
};
