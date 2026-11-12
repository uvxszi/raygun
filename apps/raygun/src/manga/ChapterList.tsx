import type { MangaChapterInfo } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { BookCheck, BookX, Check, CheckCheck, Download, ListChecks, Trash2 } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { ConfirmButton } from '../shell/ConfirmButton';
import { DownloadButton } from './DownloadButton';
import { ROUTES } from './medium';
import type { ReadingMedium } from './medium';
import {
  useDownloadChapters,
  useMarkPreviousRead,
  useRemoveDownloads,
  useSetChapterRead,
  useSetChaptersRead,
} from './useManga';

/** Rows drawn at first and added as the list's end comes near: titles can have thousands. */
const ROWS_AT_ONCE = 100;

/** Where a partly read chapter stands: a page for manga, a share of the text for novels. */
const progressLabel = (
  chapter: MangaChapterInfo,
  medium: ReadingMedium,
  t: (key: string, options?: Record<string, unknown>) => string,
) => {
  if (chapter.lastPage === null || !chapter.totalPages) return null;
  return medium === 'novel'
    ? t('novel.percentRead', {
        percent: Math.min(99, Math.round((chapter.lastPage / chapter.totalPages) * 100)),
      })
    : t('manga.pageOf', { page: chapter.lastPage + 1, total: chapter.totalPages });
};

const ChapterRow: FC<{
  titleId: string;
  chapter: MangaChapterInfo;
  medium: ReadingMedium;
  downloadable: boolean;
  selecting: boolean;
  selected: boolean;
  busy: boolean;
  onToggleRead: (chapter: MangaChapterInfo) => void;
  onSelect: (chapter: MangaChapterInfo, range: boolean) => void;
}> = ({
  titleId,
  chapter,
  medium,
  downloadable,
  selecting,
  selected,
  busy,
  onToggleRead,
  onSelect,
}) => {
  const { t, i18n } = useTranslation();
  const date = chapter.dateUpload
    ? new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(chapter.dateUpload)
    : null;
  const onClick = (event: MouseEvent) => {
    if (!selecting) return;
    event.preventDefault();
    onSelect(chapter, event.shiftKey);
  };
  return (
    <div
      className={`flex items-center gap-2 rounded-lg ${selected ? 'bg-primary/15' : ''}`}
      // A long press (or right click) starts selecting; another one selects up to it (Mihon).
      onContextMenu={(event) => {
        event.preventDefault();
        onSelect(chapter, selecting);
      }}
    >
      <Link
        to={ROUTES[medium].read}
        search={{ title: titleId, chapter: chapter.id }}
        data-testid="chapter-row"
        aria-pressed={selecting ? selected : undefined}
        onClick={onClick}
        className={`flex min-w-0 flex-1 items-center gap-2 py-2 pl-1 ${chapter.read && !selected ? 'opacity-50' : ''}`}
      >
        {selecting && (
          <span
            aria-hidden
            className={`grid size-5 shrink-0 place-items-center rounded border-2 ${selected ? 'border-primary bg-primary text-background' : 'border-muted'}`}
          >
            {selected && <Check size={14} strokeWidth={3} />}
          </span>
        )}
        <span className="flex min-w-0 flex-col">
          <span className="truncate text-sm font-medium">{chapter.name}</span>
          <span className="text-muted truncate text-xs">
            {[date, chapter.scanlator, progressLabel(chapter, medium, t)]
              .filter(Boolean)
              .join(' · ')}
          </span>
        </span>
      </Link>
      {!selecting && (
        <>
          <button
            type="button"
            aria-label={t(chapter.read ? 'manga.markUnread' : 'manga.markRead', {
              name: chapter.name,
            })}
            aria-pressed={chapter.read}
            disabled={busy}
            onClick={() => onToggleRead(chapter)}
            className={`grid size-9 shrink-0 place-items-center rounded-full ${chapter.read ? 'text-primary' : 'text-muted'}`}
          >
            <Check aria-hidden size={18} />
          </button>
          {downloadable && <DownloadButton chapter={chapter} />}
        </>
      )}
    </div>
  );
};

/** The bottom bar while chapters are selected: what to do with them. */
const ChapterActions: FC<{
  medium: ReadingMedium;
  titleId: string;
  selected: MangaChapterInfo[];
  downloadable: boolean;
  onDone: () => void;
}> = ({ medium, titleId, selected, downloadable, onDone }) => {
  const { t } = useTranslation();
  const setRead = useSetChaptersRead(medium);
  const previous = useMarkPreviousRead(medium);
  const download = useDownloadChapters();
  const remove = useRemoveDownloads();
  const finish = { onSuccess: onDone };
  const ids = selected.map((chapter) => chapter.id);
  // Oldest first, so the queue reads in order (the list is newest first).
  const toDownload = selected
    .filter((chapter) => !chapter.download)
    .map((chapter) => chapter.id)
    .reverse();
  const downloaded = selected.filter((chapter) => chapter.download).map((chapter) => chapter.id);
  const only = selected.length === 1 ? selected[0] : undefined;
  const error = setRead.error ?? previous.error ?? download.error ?? remove.error;
  const busy = setRead.isPending || previous.isPending || download.isPending || remove.isPending;

  const action = (label: string, Icon: LucideIcon, onClick: () => void) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      disabled={busy}
      onClick={onClick}
      className="hover:bg-background grid size-10 shrink-0 place-items-center rounded-full disabled:opacity-50"
    >
      <Icon aria-hidden size={20} />
    </button>
  );

  return (
    <>
      {error && <SourceIssue compact>{error.message}</SourceIssue>}
      <div
        role="toolbar"
        aria-label={t('chapters.actions')}
        data-testid="chapter-actions"
        className="bg-surface motion-rise sticky bottom-2 z-30 mt-auto flex w-full justify-around rounded-2xl p-1 shadow-xl"
      >
        {action(t('library.select.markRead'), BookCheck, () =>
          setRead.mutate({ chapterIds: ids, read: true }, finish),
        )}
        {action(t('library.select.markUnread'), BookX, () =>
          setRead.mutate({ chapterIds: ids, read: false }, finish),
        )}
        {only &&
          action(t('chapters.markPreviousRead'), CheckCheck, () =>
            previous.mutate({ titleId, chapterId: only.id }, finish),
          )}
        {downloadable &&
          toDownload.length > 0 &&
          action(t('library.select.download'), Download, () => download.mutate(toDownload, finish))}
        {downloadable && downloaded.length > 0 && (
          <ConfirmButton
            label={t('chapters.deleteDownloads')}
            confirm={t('chapters.deleteDownloadsConfirm', { count: downloaded.length })}
            icon={Trash2}
            iconSize={20}
            disabled={busy}
            onConfirm={() => remove.mutate(downloaded, finish)}
            idleClassName="hover:bg-background size-10"
          />
        )}
      </div>
    </>
  );
};

/**
 * A title's chapters in the chosen order, a hundred rows at a time. A long press selects (Mihon):
 * then rows toggle, and the bar marks them read or unread, marks the ones before read, downloads
 * them or deletes their downloads.
 */
export const ChapterList: FC<{
  titleId: string;
  medium: ReadingMedium;
  chapters: MangaChapterInfo[];
  downloadable: boolean;
  selecting: boolean;
  selected: Set<string>;
  /** Selects (and starts selecting). */
  onSelectedChange: (selected: Set<string>) => void;
  /** Stops selecting, after an action. */
  onDone: () => void;
}> = ({
  titleId,
  medium,
  chapters,
  downloadable,
  selecting,
  selected,
  onSelectedChange,
  onDone,
}) => {
  const [count, setCount] = useState(ROWS_AT_ONCE);
  const sentinel = useRef<HTMLLIElement>(null);
  const lastPicked = useRef<string | null>(null);
  const setRead = useSetChapterRead(medium);
  const shown = chapters.slice(0, count);

  useEffect(() => {
    const element = sentinel.current;
    if (!element || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setCount((rows) => rows + ROWS_AT_ONCE);
        }
      },
      { rootMargin: '800px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [count, chapters.length]);

  const onSelect = (chapter: MangaChapterInfo, range: boolean) => {
    const next = new Set(selected);
    const from =
      range && lastPicked.current
        ? chapters.findIndex((item) => item.id === lastPicked.current)
        : -1;
    const to = chapters.findIndex((item) => item.id === chapter.id);
    if (from >= 0 && to >= 0 && from !== to) {
      const [low, high] = from < to ? [from, to] : [to, from];
      for (const item of chapters.slice(low, high + 1)) next.add(item.id);
    } else if (next.has(chapter.id)) {
      next.delete(chapter.id);
    } else {
      next.add(chapter.id);
    }
    lastPicked.current = chapter.id;
    onSelectedChange(next);
  };

  return (
    <>
      <ul className="flex flex-col">
        {shown.map((chapter) => (
          <li key={chapter.id}>
            <ChapterRow
              titleId={titleId}
              chapter={chapter}
              medium={medium}
              downloadable={downloadable}
              selecting={selecting}
              selected={selected.has(chapter.id)}
              busy={setRead.isPending}
              onToggleRead={(row) => setRead.mutate({ chapterId: row.id, read: !row.read })}
              onSelect={onSelect}
            />
          </li>
        ))}
        {count < chapters.length && <li ref={sentinel} aria-hidden className="h-px" />}
      </ul>
      {selecting && selected.size > 0 && (
        <ChapterActions
          medium={medium}
          titleId={titleId}
          selected={chapters.filter((chapter) => selected.has(chapter.id))}
          downloadable={downloadable}
          onDone={onDone}
        />
      )}
    </>
  );
};

/** The chapter list's header button that starts selecting. */
export const SelectChaptersButton: FC<{ onClick: () => void }> = ({ onClick }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t('chapters.select')}
      title={t('chapters.select')}
      onClick={onClick}
      className="hover:bg-surface text-muted grid size-9 place-items-center rounded-full"
    >
      <ListChecks aria-hidden size={18} />
    </button>
  );
};
