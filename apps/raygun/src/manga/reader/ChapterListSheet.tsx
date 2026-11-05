import { commands } from '@raygun/bindings';
import type { MangaChapterInfo } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { ArrowDownToLine, BookOpen, Check } from 'lucide-react';
import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../../lib/commandResult';
import { Sheet } from '../../shell/Sheet';
import { SourceIssue } from '../../shell/SourceIssue';
import { ChapterOrderButton } from '../ChapterOrderButton';
import { orderChapters, useChapterOrder } from '../chapterOrder';
import type { ReadingMedium } from '../medium';

/**
 * The title's chapters inside a reader, the comic reader's and the book reader's alike: sorted
 * and filtered as on its page (the same order, kept per title), the one being read marked and in
 * view; picking one reads it in place. An imported book lists its table of contents.
 */
export const ChapterListSheet: FC<{
  titleId: string;
  currentId: string;
  /** A book notes how far a chapter was read as a share of its text, a comic as its page. */
  medium?: ReadingMedium;
  anchor: HTMLElement | null;
  onPick: (chapterId: string) => void;
  onClose: () => void;
}> = ({ titleId, currentId, medium = 'manga', anchor, onPick, onClose }) => {
  const { t } = useTranslation();
  const chapters = useQuery({
    queryKey: [medium, 'chapters', titleId],
    // A stored title's chapters, a book's as well as a comic's.
    queryFn: () => unwrapResult(commands.mangaChapters(titleId)),
  });
  /** Where a partly read chapter stopped: its page, or the share of a book's text read. */
  const stoppedAt = (chapter: MangaChapterInfo) => {
    if (chapter.read || chapter.lastPage === null) return null;
    if (medium === 'manga') return t('reader.chapters.page', { page: chapter.lastPage + 1 });
    return chapter.totalPages
      ? t('novel.percentRead', {
          percent: Math.min(99, Math.round((chapter.lastPage / chapter.totalPages) * 100)),
        })
      : null;
  };
  const [order, setOrder] = useChapterOrder(titleId);
  const shown = orderChapters(chapters.data ?? [], order);
  const currentRow = useRef<HTMLButtonElement>(null);
  const loaded = chapters.data !== undefined;
  useEffect(() => {
    if (loaded) currentRow.current?.scrollIntoView?.({ block: 'center' });
  }, [loaded]);

  return (
    <Sheet title={t('reader.chapters.title')} anchor={anchor} popoverWidth={360} onClose={onClose}>
      <div data-testid="reader-chapters" className="-mx-2 flex flex-col gap-1">
        <div className="flex items-center justify-between gap-2 px-2">
          <span className="text-muted text-sm">
            {loaded && t('manga.chapters', { count: chapters.data?.length ?? 0 })}
          </span>
          {(chapters.data?.length ?? 0) > 1 && (
            <ChapterOrderButton order={order} onChange={setOrder} />
          )}
        </div>
        {chapters.error && (
          <SourceIssue compact onRetry={() => void chapters.refetch()}>
            {chapters.error.message}
          </SourceIssue>
        )}
        {loaded && shown.length === 0 && (
          <p className="text-muted px-2 py-3 text-sm">{t('chapterOrder.noneShown')}</p>
        )}
        <ul className="flex max-h-[60dvh] flex-col overflow-y-auto overscroll-contain">
          {shown.map((chapter) => {
            const current = chapter.id === currentId;
            const stopped = stoppedAt(chapter);
            return (
              <li key={chapter.id}>
                <button
                  ref={current ? currentRow : undefined}
                  type="button"
                  aria-current={current ? 'true' : undefined}
                  data-testid="reader-chapter"
                  onClick={() => (current ? onClose() : onPick(chapter.id))}
                  className={`hover:bg-background flex w-full items-center gap-3 rounded-lg px-2 py-2.5 text-left text-sm ${
                    current ? 'bg-primary/15 text-primary font-semibold' : ''
                  } ${chapter.read && !current ? 'text-muted' : ''}`}
                >
                  <span className="grid size-5 shrink-0 place-items-center">
                    {current ? (
                      <BookOpen aria-hidden size={16} />
                    ) : chapter.read ? (
                      <Check aria-label={t('reader.chapters.read')} size={16} />
                    ) : null}
                  </span>
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate">{chapter.name}</span>
                    {stopped && <span className="text-muted text-xs">{stopped}</span>}
                  </span>
                  {chapter.download?.state === 'done' && (
                    <ArrowDownToLine
                      aria-label={t('reader.chapters.downloaded')}
                      size={16}
                      className="text-muted shrink-0"
                    />
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      </div>
    </Sheet>
  );
};
