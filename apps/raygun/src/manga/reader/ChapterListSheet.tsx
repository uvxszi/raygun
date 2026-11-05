import { commands } from '@raygun/bindings';
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
import { MANGA } from '../useManga';

/**
 * The title's chapters inside the reader: sorted and filtered as on its page (the same order,
 * kept per title), the one being read marked and in view; picking one reads it in place.
 */
export const ChapterListSheet: FC<{
  titleId: string;
  currentId: string;
  anchor: HTMLElement | null;
  onPick: (chapterId: string) => void;
  onClose: () => void;
}> = ({ titleId, currentId, anchor, onPick, onClose }) => {
  const { t } = useTranslation();
  const chapters = useQuery({
    queryKey: [MANGA, 'chapters', titleId],
    queryFn: () => unwrapResult(commands.mangaChapters(titleId)),
  });
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
                      <Check aria-hidden size={16} />
                    ) : null}
                  </span>
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate">{chapter.name}</span>
                    {chapter.lastPage !== null && !chapter.read && (
                      <span className="text-muted text-xs">
                        {t('reader.chapters.page', { page: chapter.lastPage + 1 })}
                      </span>
                    )}
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
