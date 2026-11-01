import type { MangaChapterInfo } from '@raygun/bindings';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { BookOpen, Check, Download, Globe, Heart, LoaderCircle, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { CategoryPicker } from '../manga/CategoryPicker';
import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { DownloadButton } from '../manga/DownloadButton';
import { MangaCover } from '../manga/MangaCover';
import { LOCAL_EPUB, ROUTES } from '../manga/medium';
import type { ReadingMedium } from '../manga/medium';
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
}> = ({ titleId, chapter, medium, downloadable }) => {
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
      {downloadable && <DownloadButton chapter={chapter} />}
    </div>
  );
};

/** A title: details, library button, continue reading and chapters. */
export const MangaTitlePage: FC<{ medium?: ReadingMedium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const { source = '', id = '' }: { source?: string; id?: string } = useSearch({ strict: false });
  useMangaLiveEvents();
  const title = useMangaTitle(source, id, medium);
  const download = useDownloadChapters();
  const refresh = useRefreshTitle(medium);
  const setInLibrary = useSetInLibrary(medium);
  const [expanded, setExpanded] = useState(false);
  const navigate = useNavigate();
  const view = refresh.data && refresh.data.remoteId === id ? refresh.data : title.data;
  const top = (
    <TopBar
      back
      fallback="/library"
      plainTitle
      title={view?.sourceName ?? ''}
      actions={
        view && (
          <TopBarAction
            label={t('manga.refresh')}
            icon={RefreshCw}
            disabled={refresh.isPending}
            onClick={() => refresh.mutate({ sourceId: source, mangaId: id })}
          />
        )
      }
    />
  );

  if (title.error && !view) {
    return (
      <section data-testid="manga-title-page">
        {top}
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {title.error.message}
        </p>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="manga-title-page">
        {top}
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted mx-auto my-12 animate-spin"
        />
      </section>
    );
  }

  if (view.hiddenNsfw) {
    return (
      <section
        data-testid="manga-title-page"
        className="flex flex-col items-center gap-3 py-12 text-center"
      >
        <h1 className="text-lg font-bold">{t('manga.nsfwHiddenTitle')}</h1>
        <p className="text-muted text-sm">{t('manga.nsfwHiddenHint')}</p>
        <Link
          to="/settings"
          className="bg-primary text-background rounded-full px-4 py-2 font-semibold"
        >
          {t('manga.openSettings')}
        </Link>
      </section>
    );
  }

  const continueChapter = view.chapters.find((chapter) => chapter.id === view.continueChapterId);
  const started = view.chapters.some((chapter) => chapter.read || chapter.lastPage !== null);
  // Imported books are already on the device.
  const downloadable = view.sourceId !== LOCAL_EPUB;
  // Oldest first, so the queue reads in order (the list is newest first).
  const unreadToDownload = view.chapters
    .filter((chapter) => downloadable && !chapter.read && !chapter.download)
    .map((chapter) => chapter.id)
    .reverse();

  return (
    <section data-testid="manga-title-page" className="flex min-h-full flex-col gap-5">
      {top}
      <TitleHero
        art={view.coverUrl}
        cover={<MangaCover url={view.coverUrl} className="w-full rounded-xl shadow-lg" />}
        title={view.title}
        genres={view.genres}
        facts={[
          [...new Set([...view.authors, ...view.artists])].join(', '),
          t(`manga.status.${view.status}`),
          view.sourceName,
        ]}
      >
        <div className="flex gap-2">
          <button
            type="button"
            aria-pressed={view.inLibrary}
            disabled={setInLibrary.isPending}
            onClick={() => setInLibrary.mutate({ titleId: view.id, inLibrary: !view.inLibrary })}
            className={`flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60 ${view.inLibrary ? 'text-primary' : ''}`}
          >
            <Heart aria-hidden size={22} fill={view.inLibrary ? 'currentColor' : 'none'} />
            {t(view.inLibrary ? 'manga.inLibrary' : 'manga.addToLibrary')}
          </button>
          {downloadable && (
            <button
              type="button"
              disabled={download.isPending || unreadToDownload.length === 0}
              onClick={() => download.mutate(unreadToDownload)}
              aria-label={
                unreadToDownload.length > 0
                  ? t('manga.downloadUnread', { count: unreadToDownload.length })
                  : undefined
              }
              className="flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60"
            >
              <span className="relative">
                <Download aria-hidden size={22} />
                {unreadToDownload.length > 0 && (
                  <span
                    aria-hidden
                    className="bg-primary text-background absolute -top-2 -right-3 min-w-5 rounded-full px-1 text-center text-[10px] leading-4 font-bold"
                  >
                    {unreadToDownload.length}
                  </span>
                )}
              </span>
              {view.chapters.length > 0 &&
              unreadToDownload.length === 0 &&
              view.chapters.every((chapter) => chapter.download)
                ? t('manga.allDownloaded')
                : t('manga.downloadAction')}
            </button>
          )}
          {view.url && (
            <a
              href={view.url}
              target="_blank"
              rel="noopener"
              className="flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60"
            >
              <Globe aria-hidden size={22} />
              {t('manga.openInBrowser')}
            </a>
          )}
        </div>
      </TitleHero>
      {view.inLibrary && <CategoryPicker medium={medium} titleId={view.id} />}
      {(refresh.error ?? setInLibrary.error ?? download.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(refresh.error ?? setInLibrary.error ?? download.error)?.message}
        </p>
      )}
      {view.description && (
        <button
          type="button"
          onClick={() => setExpanded((open) => !open)}
          aria-expanded={expanded}
          className={`text-left text-sm leading-relaxed whitespace-pre-line ${expanded ? '' : 'line-clamp-4'}`}
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
                <ChapterRow
                  titleId={view.id}
                  chapter={chapter}
                  medium={medium}
                  downloadable={downloadable}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
      {continueChapter && (
        <ResumeFab
          icon={BookOpen}
          label={started ? t('manga.continue', { name: continueChapter.name }) : t('manga.start')}
          onClick={() =>
            void navigate({
              to: ROUTES[medium].read,
              search: { title: view.id, chapter: continueChapter.id },
            })
          }
        />
      )}
    </section>
  );
};

export const NovelTitlePage: FC = () => <MangaTitlePage medium="novel" />;
