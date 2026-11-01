import type { MangaChapterInfo } from '@raygun/bindings';
import { Link, useSearch } from '@tanstack/react-router';
import { BookOpen, Check, Download, LoaderCircle, Minus, Plus, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { DownloadButton } from '../manga/DownloadButton';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import {
  useDownloadChapters,
  useMangaLiveEvents,
  useMangaTitle,
  useRefreshTitle,
  useSetChapterRead,
  useSetInLibrary,
} from '../manga/useManga';

/** Where a partly read chapter stands: a page for manga, a share of the text for novels. */
const progressLabel = (
  chapter: MangaChapterInfo,
  medium: Medium,
  t: (key: string, options?: Record<string, unknown>) => string,
) => {
  if (chapter.lastPage === null || !chapter.totalPages) return null;
  return medium === 'novel'
    ? t('novel.percentRead', {
        percent: Math.min(99, Math.round((chapter.lastPage / chapter.totalPages) * 100)),
      })
    : t('manga.pageOf', { page: chapter.lastPage + 1, total: chapter.totalPages });
};

const ChapterRow: FC<{ titleId: string; chapter: MangaChapterInfo; medium: Medium }> = ({
  titleId,
  chapter,
  medium,
}) => {
  const { t, i18n } = useTranslation();
  const setRead = useSetChapterRead(medium);
  const date = chapter.dateUpload
    ? new Intl.DateTimeFormat(i18n.language, { dateStyle: 'medium' }).format(chapter.dateUpload)
    : null;
  return (
    <div className="flex items-center gap-2">
      <Link
        to={ROUTES[medium].read}
        search={{ title: titleId, chapter: chapter.id }}
        data-testid="chapter-row"
        className={`flex min-w-0 flex-1 flex-col py-2 ${chapter.read ? 'opacity-50' : ''}`}
      >
        <span className="truncate text-sm font-medium">{chapter.name}</span>
        <span className="text-muted truncate text-xs">
          {[date, chapter.scanlator, progressLabel(chapter, medium, t)].filter(Boolean).join(' · ')}
        </span>
      </Link>
      <button
        type="button"
        aria-label={t(chapter.read ? 'manga.markUnread' : 'manga.markRead', { name: chapter.name })}
        aria-pressed={chapter.read}
        disabled={setRead.isPending}
        onClick={() => setRead.mutate({ chapterId: chapter.id, read: !chapter.read })}
        className={`grid size-9 shrink-0 place-items-center rounded-full ${chapter.read ? 'text-primary' : 'text-muted'}`}
      >
        <Check aria-hidden size={18} />
      </button>
      {medium === 'manga' && <DownloadButton chapter={chapter} />}
    </div>
  );
};

/** A title: details, library button, continue reading and chapters. */
export const MangaTitlePage: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const { source = '', id = '' }: { source?: string; id?: string } = useSearch({ strict: false });
  useMangaLiveEvents();
  const title = useMangaTitle(source, id, medium);
  const download = useDownloadChapters();
  const refresh = useRefreshTitle(medium);
  const setInLibrary = useSetInLibrary(medium);
  const [expanded, setExpanded] = useState(false);
  const view = refresh.data && refresh.data.remoteId === id ? refresh.data : title.data;

  if (title.error && !view) {
    return (
      <section data-testid="manga-title-page">
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {title.error.message}
        </p>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="manga-title-page" className="grid place-items-center py-12">
        <LoaderCircle aria-label={t('manga.loading')} className="text-muted animate-spin" />
      </section>
    );
  }

  const continueChapter = view.chapters.find((chapter) => chapter.id === view.continueChapterId);
  const started = view.chapters.some((chapter) => chapter.read || chapter.lastPage !== null);
  // Oldest first, so the queue reads in order (the list is newest first).
  const unreadToDownload = view.chapters
    .filter((chapter) => medium === 'manga' && !chapter.read && !chapter.download)
    .map((chapter) => chapter.id)
    .reverse();

  return (
    <section data-testid="manga-title-page" className="flex flex-col gap-5">
      <header className="flex gap-4">
        <MangaCover url={view.coverUrl} className="w-28 shrink-0 rounded-lg shadow-lg sm:w-40" />
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="text-xl font-bold tracking-tight wrap-anywhere">{view.title}</h1>
          {view.authors.length > 0 && (
            <p className="text-muted text-sm">
              {[...new Set([...view.authors, ...view.artists])].join(', ')}
            </p>
          )}
          <p className="text-muted text-xs">
            {[t(`manga.status.${view.status}`), view.sourceName].join(' · ')}
          </p>
        </div>
      </header>
      <div className="flex flex-wrap gap-3">
        {continueChapter && (
          <Link
            to={ROUTES[medium].read}
            search={{ title: view.id, chapter: continueChapter.id }}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 font-semibold"
          >
            <BookOpen aria-hidden size={18} />
            {started ? t('manga.continue', { name: continueChapter.name }) : t('manga.start')}
          </Link>
        )}
        <button
          type="button"
          disabled={setInLibrary.isPending}
          onClick={() => setInLibrary.mutate({ titleId: view.id, inLibrary: !view.inLibrary })}
          className="bg-surface flex items-center gap-2 rounded-full px-5 py-2.5 disabled:opacity-60"
        >
          {view.inLibrary ? <Minus aria-hidden size={18} /> : <Plus aria-hidden size={18} />}
          {t(view.inLibrary ? 'manga.removeFromLibrary' : 'manga.addToLibrary')}
        </button>
        <button
          type="button"
          aria-label={t('manga.refresh')}
          disabled={refresh.isPending}
          onClick={() => refresh.mutate({ sourceId: source, mangaId: id })}
          className="text-muted grid size-11 place-items-center rounded-full"
        >
          <RefreshCw aria-hidden size={18} className={refresh.isPending ? 'animate-spin' : ''} />
        </button>
        {unreadToDownload.length > 0 && (
          <button
            type="button"
            disabled={download.isPending}
            onClick={() => download.mutate(unreadToDownload)}
            className="bg-surface flex items-center gap-2 rounded-full px-5 py-2.5 disabled:opacity-60"
          >
            <Download aria-hidden size={18} />
            {t('manga.downloadUnread', { count: unreadToDownload.length })}
          </button>
        )}
      </div>
      {(refresh.error ?? setInLibrary.error ?? download.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(refresh.error ?? setInLibrary.error ?? download.error)?.message}
        </p>
      )}
      {view.genres.length > 0 && (
        <ul className="flex flex-wrap gap-1.5">
          {view.genres.map((genre) => (
            <li key={genre} className="bg-surface rounded-full px-3 py-1 text-xs">
              {genre}
            </li>
          ))}
        </ul>
      )}
      {view.description && (
        <button
          type="button"
          onClick={() => setExpanded((open) => !open)}
          aria-expanded={expanded}
          className={`text-muted text-left text-sm whitespace-pre-line ${expanded ? '' : 'line-clamp-4'}`}
        >
          {view.description}
        </button>
      )}
      <div className="flex flex-col gap-1">
        <h2 className="font-semibold">{t('manga.chapters', { count: view.chapters.length })}</h2>
        {view.chapters.length === 0 ? (
          <p className="text-muted text-sm">{t('manga.noChapters')}</p>
        ) : (
          <ul className="flex flex-col">
            {view.chapters.map((chapter) => (
              <li key={chapter.id}>
                <ChapterRow titleId={view.id} chapter={chapter} medium={medium} />
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
};

export const NovelTitlePage: FC = () => <MangaTitlePage medium="novel" />;
