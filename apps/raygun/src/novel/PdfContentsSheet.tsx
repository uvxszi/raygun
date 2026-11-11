import type { PdfContentsEntry } from '@raygun/bindings';
import { ChevronRight } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { currentEntry, outlineTree } from './pdfContents';

/** How far a level is set in, and the deepest indented. */
const INDENT_REM = 1;
const MAX_INDENT = 6;

/**
 * A PDF book's contents (its outline), the same in its page and text views: a tree whose branches
 * open and close, the part being read marked, shown and its branch open. Picking a bookmark goes
 * to its page, in this chapter or another.
 */
export const PdfContentsSheet: FC<{
  contents: PdfContentsEntry[];
  /** The document's number (from 1) of the page being read. */
  pageNumber: number;
  /** The button that opened it, for a popover beside it on wide windows. */
  anchor?: HTMLElement | null;
  onPick: (entry: PdfContentsEntry) => void;
  onClose: () => void;
}> = ({ contents, pageNumber, anchor, onPick, onClose }) => {
  const { t } = useTranslation();
  const tree = useMemo(() => outlineTree(contents), [contents]);
  const current = useMemo(() => currentEntry(contents, pageNumber), [contents, pageNumber]);
  // Open at first: the branches leading to the part being read, and a lone top entry (the
  // book's own title) so its chapters show.
  const [open, setOpen] = useState(() => {
    const opened = new Set<number>();
    let at: number | null = current >= 0 ? current : null;
    while (at !== null) {
      opened.add(at);
      at = tree.parents[at] ?? null;
    }
    const roots = tree.parents.flatMap((parent, index) => (parent === null ? [index] : []));
    if (roots.length === 1) opened.add(roots[0]!);
    return opened;
  });
  const shown = useMemo(() => {
    const visible: boolean[] = [];
    tree.parents.forEach((parent, index) => {
      visible[index] = parent === null || (visible[parent] === true && open.has(parent));
    });
    return contents.flatMap((entry, index) => (visible[index] ? [{ entry, index }] : []));
  }, [contents, tree, open]);
  const currentRow = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    currentRow.current?.scrollIntoView?.({ block: 'center' });
  }, []);
  const toggle = (index: number) =>
    setOpen((before) => {
      const after = new Set(before);
      if (!after.delete(index)) after.add(index);
      return after;
    });

  return (
    <Sheet title={t('pdf.contents')} anchor={anchor} popoverWidth={380} onClose={onClose}>
      <ul
        data-testid="pdf-contents"
        className="-mx-2 flex max-h-[60dvh] flex-col overflow-y-auto overscroll-contain"
      >
        {shown.map(({ entry, index }) => {
          const reading = index === current;
          const branch = tree.hasChildren[index];
          const expanded = open.has(index);
          return (
            <li
              key={index}
              className="flex items-center"
              style={{
                paddingLeft: `${Math.min(tree.depths[index] ?? 0, MAX_INDENT) * INDENT_REM}rem`,
              }}
            >
              {branch ? (
                <button
                  type="button"
                  aria-label={t('pdf.sectionsOf', { title: entry.title })}
                  aria-expanded={expanded}
                  onClick={() => toggle(index)}
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
                ref={reading ? currentRow : undefined}
                type="button"
                aria-current={reading ? 'true' : undefined}
                aria-label={t('pdf.contentsEntry', { title: entry.title, page: entry.pageNumber })}
                onClick={() => onPick(entry)}
                className={`hover:bg-background flex min-w-0 flex-1 items-baseline gap-3 rounded-lg py-2 pr-3 pl-1 text-left text-sm ${
                  reading ? 'bg-primary/15 text-primary font-semibold' : ''
                } ${(tree.depths[index] ?? 0) === 0 ? 'font-medium' : ''}`}
              >
                <span className="min-w-0 flex-1 [overflow-wrap:anywhere]">{entry.title}</span>
                <span className="text-muted shrink-0 text-xs tabular-nums">{entry.pageNumber}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </Sheet>
  );
};
