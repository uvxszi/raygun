import { commands } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { BookOpen, Globe, Heart, LoaderCircle, RefreshCw } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { CategoryPicker } from '../manga/CategoryPicker';
import { ChapterList, SelectChaptersButton } from '../manga/ChapterList';
import { useKnownCard } from '../manga/knownCard';
import { ChapterOrderButton } from '../manga/ChapterOrderButton';
import { orderChapters, useChapterOrder } from '../manga/chapterOrder';
import { DownloadMenu } from '../manga/DownloadMenu';
import { SelectionTopBar } from '../manga/LibrarySelection';
import { FindInOtherSourcesButton, OtherSources } from '../manga/OtherSources';
import { unwrapResult } from '../lib/commandResult';
import { ResumeFab, TitleHero } from '../shell/TitleHero';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';
import { bookWords } from '../manga/bookWords';
import { MangaCover } from '../manga/MangaCover';
import { ExportButton } from '../export/ExportButton';
import { TitleListActions } from '../manga/TitleListActions';
import { useDuplicateCheck } from '../manga/useDuplicateCheck';
import { ROUTES, isLocalSource } from '../manga/medium';
import type { ReadingMedium } from '../manga/medium';
import {
  useMangaLiveEvents,
  useMangaTitle,
  useRefreshTitle,
  useSetInLibrary,
} from '../manga/useManga';
import { SourceIssue } from '../shell/SourceIssue';

const NONE: Set<string> = new Set();

/** A title: details, library button, continue reading and chapters. */
export const MangaTitlePage: FC<{ medium?: ReadingMedium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const { source = '', id = '' }: { source?: string; id?: string } = useSearch({ strict: false });
  useMangaLiveEvents();
  const title = useMangaTitle(source, id, medium);
  const refresh = useRefreshTitle(medium);
  const setInLibrary = useSetInLibrary(medium);
  const duplicates = useDuplicateCheck(medium);
  const [expanded, setExpanded] = useState(false);
  const [finding, setFinding] = useState(false);
  // Selection mode and the chapters picked in it, for the title they belong to.
  const [selection, setSelection] = useState<{ key: string; ids: Set<string> } | null>(null);
  const navigate = useNavigate();
  const view = refresh.data && refresh.data.remoteId === id ? refresh.data : title.data;
  const titleKey = `${source}:${id}`;
  const selecting = selection?.key === titleKey;
  const selected = selecting ? selection.ids : NONE;
  const setSelected = (ids: Set<string>) => setSelection({ key: titleKey, ids });
  const stopSelecting = () => setSelection(null);
  const [order, setOrder] = useChapterOrder(view?.id ?? '');
  // What the search or list already showed (M46.38): the header shows at once, and stays when
  // the source then fails.
  const known = useKnownCard(medium, source, id);
  const knownHero = known && (
    <TitleHero
      art={known.coverUrl}
      cover={
        <MangaCover
          url={known.coverUrl}
          book={bookWords(medium, known.title, known.authors)}
          className="w-full rounded-xl shadow-lg"
        />
      }
      title={known.title}
    />
  );
  // The website button (M46.38) opens the title's page in Raygun's browser: a plain link does
  // nothing in the app's window.
  const website = useMutation({
    mutationFn: () =>
      unwrapResult(commands.catalogOpenPage(view?.sourceId ?? source, view?.url ?? null)),
  });

  // Escape stops selecting.
  useEffect(() => {
    if (!selecting) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setSelection(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [selecting]);

  const top = (
    <TopBar
      back
      fallback="/library"
      plainTitle
      title={view?.sourceName ?? ''}
      actions={
        view && (
          <>
            <PinButton
              target={{
                kind: medium,
                to: medium === 'novel' ? '/novel/title' : '/manga/title',
                search: { source, id },
                name: view.title,
                artworkUrl: view.coverUrl ?? null,
              }}
            />
            {!isLocalSource(view.sourceId) && (
              <TopBarAction
                label={t('manga.refresh')}
                icon={RefreshCw}
                disabled={refresh.isPending}
                onClick={() => refresh.mutate({ sourceId: source, mangaId: id })}
              />
            )}
            <ExportButton
              medium={medium}
              titleIds={[view.id]}
              // The list is newest first; ranges and boxes go in reading order.
              chapters={[...view.chapters].reverse().map((chapter) => ({
                id: chapter.id,
                name: chapter.name,
                downloaded: chapter.download?.state === 'done',
                read: chapter.read,
              }))}
            />
          </>
        )
      }
    />
  );

  if (title.error && !view) {
    return (
      <section data-testid="manga-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
        <SourceIssue onRetry={() => void title.refetch()}>{title.error.message}</SourceIssue>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="manga-title-page" className="flex flex-col gap-5">
        {top}
        {knownHero}
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
  // Once everything is read there's nothing to continue (Mihon hides the button).
  const finished = view.chapters.length > 0 && view.chapters.every((chapter) => chapter.read);
  // Imported books are already on the device.
  const downloadable = !isLocalSource(view.sourceId);
  const shown = orderChapters(view.chapters, order);
  const issue = refresh.error ?? setInLibrary.error ?? website.error;

  return (
    <section data-testid="manga-title-page" className="flex min-h-full flex-col gap-5">
      {selecting ? (
        <SelectionTopBar
          count={selected.size}
          onSelectAll={() =>
            setSelected(new Set([...selected, ...shown.map((chapter) => chapter.id)]))
          }
          onInvert={() =>
            setSelected(
              new Set(
                shown.map((chapter) => chapter.id).filter((chapterId) => !selected.has(chapterId)),
              ),
            )
          }
          onClose={stopSelecting}
        />
      ) : (
        top
      )}
      <TitleHero
        art={view.coverUrl}
        cover={
          <MangaCover
            url={view.coverUrl}
            book={bookWords(medium, view.title, view.authors)}
            className="w-full rounded-xl shadow-lg"
          />
        }
        title={view.title}
        adult={view.adult}
        genres={view.genres}
        facts={[
          [...new Set([...view.authors, ...view.artists])].join(', '),
          t(`manga.status.${view.status}`),
        ]}
        source={view.sourceName}
      >
        <div className="flex gap-2">
          <button
            type="button"
            aria-pressed={view.inLibrary}
            disabled={setInLibrary.isPending || duplicates.checking}
            onClick={() => {
              const add = () => setInLibrary.mutate({ titleId: view.id, inLibrary: true });
              if (view.inLibrary) setInLibrary.mutate({ titleId: view.id, inLibrary: false });
              else void duplicates.addChecked(view.id, view.title, add);
            }}
            className={`flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60 ${view.inLibrary ? 'text-primary' : ''}`}
          >
            <Heart aria-hidden size={22} fill={view.inLibrary ? 'currentColor' : 'none'} />
            {t(view.inLibrary ? 'manga.inLibrary' : 'manga.addToLibrary')}
          </button>
          <TitleListActions medium={medium} titleId={view.id} title={view.title} />
          {downloadable && (
            <DownloadMenu medium={medium} titleId={view.id} chapters={view.chapters} />
          )}
          {!isLocalSource(view.sourceId) && (
            <button
              type="button"
              disabled={website.isPending}
              onClick={() => website.mutate()}
              className="flex flex-1 flex-col items-center gap-1 rounded-xl px-2 py-2 text-xs font-medium disabled:opacity-60"
            >
              <Globe aria-hidden size={22} />
              {t('manga.openInBrowser')}
            </button>
          )}
        </div>
      </TitleHero>
      {view.inLibrary && <CategoryPicker medium={medium} titleId={view.id} />}
      {duplicates.dialog}
      {issue && (
        <SourceIssue
          source={view.sourceName}
          onRetry={
            refresh.error ? () => refresh.mutate({ sourceId: source, mangaId: id }) : undefined
          }
        >
          {issue.message}
        </SourceIssue>
      )}
      {/* An imported comic or book can be found online too; only a source's title moves. */}
      <FindInOtherSourcesButton open={finding} onToggle={() => setFinding(!finding)} />
      {finding && (
        <OtherSources
          medium={medium}
          title={view.title}
          author={view.authors[0] ?? null}
          sourceId={view.sourceId}
          titleId={isLocalSource(view.sourceId) ? undefined : view.id}
          inLibrary={view.inLibrary}
        />
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
        <div className="flex items-center justify-between gap-2">
          <h2 className="font-semibold">{t('manga.chapters', { count: view.chapters.length })}</h2>
          <span className="flex items-center gap-1">
            {view.chapters.length > 0 && !selecting && (
              <SelectChaptersButton onClick={() => setSelected(new Set())} />
            )}
            {view.chapters.length > 1 && <ChapterOrderButton order={order} onChange={setOrder} />}
          </span>
        </div>
        {view.chapters.length === 0 ? (
          <p className="text-muted text-sm">{t('manga.noChapters')}</p>
        ) : shown.length === 0 ? (
          <p className="text-muted text-sm">{t('chapterOrder.noneShown')}</p>
        ) : (
          <ChapterList
            titleId={view.id}
            medium={medium}
            chapters={shown}
            downloadable={downloadable}
            selecting={selecting}
            selected={selected}
            onSelectedChange={setSelected}
            onDone={stopSelecting}
          />
        )}
      </div>
      {continueChapter && !finished && !selecting && (
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
