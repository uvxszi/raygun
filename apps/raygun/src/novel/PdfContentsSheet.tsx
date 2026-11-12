import { commands } from '@raygun/bindings';
import type { PdfContentsEntry } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Check, ChevronRight } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { afterLayersClose } from '../shell/backLayers';
import { Sheet } from '../shell/Sheet';
import { bookContents, currentRow, levelTree, placeOf } from './pdfContents';
import type { ContentsPlace, ContentsRow } from './pdfContents';

/** How far a level is set in, and the deepest indented. */
const INDENT_REM = 1;
const MAX_INDENT = 6;

/**
 * A PDF book's contents, the one list of its page and text views alike: its outline as a tree
 * whose branches open and close, with its chapters (those no bookmark leads into, or every one of
 * a source's chapters), the part being read marked, shown and its branch open. Picking a bookmark
 * goes to its page, a chapter to its start, in this chapter or another: the page there in the
 * page view, its heading in the text view.
 */
export const PdfContentsSheet: FC<{
  contents: PdfContentsEntry[];
  titleId: string;
  /** The chapter being read. */
  chapterId: string;
  /** The document's number (from 1) of the page being read. */
  pageNumber: number;
  /** The button that opened it, for a popover beside it on wide windows. */
  anchor?: HTMLElement | null;
  onPick: (place: ContentsPlace) => void;
  onClose: () => void;
}> = ({ contents, titleId, chapterId, pageNumber, anchor, onPick, onClose }) => {
  const { t } = useTranslation();
  // The book's chapters, as the chapter list has them; the outline alone until they're here.
  const chapters = useQuery({
    queryKey: ['novel', 'chapters', titleId],
    queryFn: () => unwrapResult(commands.mangaChapters(titleId)),
  });
  const rows = useMemo(
    () => bookContents(contents, chapters.data ?? []),
    [contents, chapters.data],
  );
  const tree = useMemo(() => levelTree(rows.map((row) => row.level)), [rows]);
  const current = useMemo(
    () => currentRow(rows, contents, chapterId, pageNumber),
    [rows, contents, chapterId, pageNumber],
  );
  // Rows by what they stand for, so branches stay open or closed as the chapters arrive.
  const keys = useMemo(() => {
    const entryIndex = new Map(contents.map((entry, index) => [entry, index]));
    return rows.map((row: ContentsRow) =>
      row.kind === 'bookmark' ? `b${entryIndex.get(row.entry)}` : `c${row.chapter.id}`,
    );
  }, [contents, rows]);
  // Open at first: the branches leading to the part being read, and a lone top row (the book's
  // own title) so its chapters show.
  const opened = useMemo(() => {
    const open = new Set<number>();
    let at: number | null = current >= 0 ? current : null;
    while (at !== null) {
      open.add(at);
      at = tree.parents[at] ?? null;
    }
    const roots = tree.parents.flatMap((parent, index) => (parent === null ? [index] : []));
    if (roots.length === 1) open.add(roots[0]!);
    return open;
  }, [current, tree]);
  /** Branches opened or closed by hand. */
  const [toggled, setToggled] = useState<Map<string, boolean>>(() => new Map());
  const shown = useMemo(() => {
    const isOpen = (index: number) => toggled.get(keys[index]!) ?? opened.has(index);
    const visible: boolean[] = [];
    tree.parents.forEach((parent, index) => {
      visible[index] = parent === null || (visible[parent] === true && isOpen(parent));
    });
    return rows.flatMap((row, index) =>
      visible[index] ? [{ row, index, open: isOpen(index) }] : [],
    );
  }, [rows, tree, keys, toggled, opened]);
  const currentRef = useRef<HTMLButtonElement>(null);
  const settled = !chapters.isPending;
  useEffect(() => {
    currentRef.current?.scrollIntoView?.({ block: 'center' });
  }, [settled]);
  const toggle = (index: number, open: boolean) =>
    setToggled((before) => new Map(before).set(keys[index]!, !open));
  /** In this chapter at once; another chapter takes the reader's place once the sheet is gone. */
  const pick = (place: ContentsPlace) => {
    if (place.chapterId === chapterId) {
      onPick(place);
      return;
    }
    onClose();
    afterLayersClose(() => onPick(place));
  };

  return (
    <Sheet title={t('pdf.contents')} anchor={anchor} popoverWidth={380} onClose={onClose}>
      <ul
        data-testid="pdf-contents"
        className="-mx-2 flex max-h-[60dvh] flex-col overflow-y-auto overscroll-contain"
      >
        {shown.map(({ row, index, open }) => {
          const reading = index === current;
          const branch = tree.hasChildren[index];
          const expanded = branch === true && open;
          const depth = tree.depths[index] ?? 0;
          const name = row.kind === 'bookmark' ? row.entry.title : row.chapter.name;
          return (
            <li
              key={keys[index]}
              className="flex items-center"
              style={{ paddingLeft: `${Math.min(depth, MAX_INDENT) * INDENT_REM}rem` }}
            >
              {branch ? (
                <button
                  type="button"
                  aria-label={t('pdf.sectionsOf', { title: name })}
                  aria-expanded={expanded}
                  onClick={() => toggle(index, open)}
                  className="hover:bg-background text-muted grid size-9 shrink-0 place-items-center rounded-full"
                >
                  <ChevronRight
                    aria-hidden
                    size={18}
                    className={`transition-transform ${expanded ? 'rotate-90' : ''}`}
                  />
                </button>
              ) : (
                <span aria-hidden className="size-9 shrink-0" />
              )}
              <button
                ref={reading ? currentRef : undefined}
                type="button"
                aria-current={reading ? 'true' : undefined}
                aria-label={
                  row.kind === 'bookmark'
                    ? t('pdf.contentsEntry', { title: name, page: row.entry.pageNumber })
                    : undefined
                }
                data-testid={row.kind === 'chapter' ? 'pdf-contents-chapter' : undefined}
                onClick={() => pick(placeOf(row))}
                className={`hover:bg-background flex min-w-0 flex-1 items-baseline gap-3 rounded-lg py-2 pr-3 pl-1 text-left text-sm ${
                  reading ? 'bg-primary/15 text-primary font-semibold' : ''
                } ${depth === 0 ? 'font-medium' : ''}`}
              >
                <span className="min-w-0 flex-1 [overflow-wrap:anywhere]">{name}</span>
                {row.kind === 'bookmark' ? (
                  <span className="text-muted shrink-0 text-xs tabular-nums">
                    {row.entry.pageNumber}
                  </span>
                ) : (
                  row.chapter.read &&
                  !reading && (
                    <Check
                      aria-label={t('reader.chapters.read')}
                      size={16}
                      className="text-muted shrink-0 self-center"
                    />
                  )
                )}
              </button>
            </li>
          );
        })}
      </ul>
    </Sheet>
  );
};
