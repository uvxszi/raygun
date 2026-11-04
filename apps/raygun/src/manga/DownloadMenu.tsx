import type { MangaChapterInfo } from '@raygun/bindings';
import { Download } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { Sheet } from '../shell/Sheet';
import type { ReadingMedium } from './medium';
import { useDownloadNext } from './useLibraryActions';
import { useDownloadChapters } from './useManga';

const NEXT = [1, 5, 10];

const ROW =
  'hover:bg-background flex items-center justify-between gap-3 rounded-xl px-3 py-3 text-left disabled:opacity-50';

/**
 * A title page's Download (Mihon's menu): the next unread chapters, every unread one or every
 * chapter not on the device yet, oldest first.
 */
export const DownloadMenu: FC<{
  medium: ReadingMedium;
  titleId: string;
  /** The title's chapters, newest first as sources list them. */
  chapters: MangaChapterInfo[];
}> = ({ medium, titleId, chapters }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const next = useDownloadNext(medium);
  const all = useDownloadChapters();
  const missing = chapters.filter((chapter) => !chapter.download);
  const unread = missing.filter((chapter) => !chapter.read);
  const everything = chapters.length > 0 && missing.length === 0;
  const pick = (amount: number | null) => {
    setAnchor(null);
    next.mutate({ ids: [titleId], amount });
  };
  const error = next.error ?? all.error;

  return (
    <>
      <button
        type="button"
        aria-haspopup="dialog"
        disabled={everything || chapters.length === 0}
        onClick={(event) => setAnchor(event.currentTarget)}
        className="flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60"
      >
        <span className="relative">
          <Download aria-hidden size={22} />
          {unread.length > 0 && (
            <span
              aria-hidden
              className="bg-primary text-background absolute -top-2 -right-3 min-w-5 rounded-full px-1 text-center text-[10px] leading-4 font-bold"
            >
              {unread.length}
            </span>
          )}
        </span>
        {everything ? t('manga.allDownloaded') : t('manga.downloadAction')}
      </button>
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
      {anchor && (
        <Sheet
          title={t('manga.downloadAction')}
          anchor={anchor}
          popoverWidth={260}
          onClose={() => setAnchor(null)}
        >
          <div className="-mx-2 flex flex-col text-sm">
            {NEXT.map((amount) => (
              <button
                key={amount}
                type="button"
                disabled={unread.length === 0}
                onClick={() => pick(amount)}
                className={ROW}
              >
                {t('library.select.downloadNext', { count: amount })}
              </button>
            ))}
            <button
              type="button"
              disabled={unread.length === 0}
              onClick={() => pick(null)}
              className={ROW}
            >
              {t('library.select.downloadUnread')}
              <span className="text-muted text-xs tabular-nums">{unread.length}</span>
            </button>
            <button
              type="button"
              onClick={() => {
                setAnchor(null);
                // Oldest first, so the queue reads in order.
                all.mutate(missing.map((chapter) => chapter.id).reverse());
              }}
              className={ROW}
            >
              {t('manga.downloadAll')}
              <span className="text-muted text-xs tabular-nums">{missing.length}</span>
            </button>
          </div>
        </Sheet>
      )}
    </>
  );
};
