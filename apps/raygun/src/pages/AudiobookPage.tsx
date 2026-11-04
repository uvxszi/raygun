import { useQueryClient } from '@tanstack/react-query';
import { useSearch } from '@tanstack/react-router';
import {
  Check,
  CheckCheck,
  Download,
  Globe,
  LoaderCircle,
  Play,
  Plus,
  RotateCcw,
  Trash2,
} from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useAudiobook,
  useDownloadAudiobook,
  useKeepAudiobook,
  usePlayAudiobook,
} from '../audiobooks/useAudiobooks';
import { EpisodeItem } from '../podcasts/EpisodeItem';
import { EpisodeList } from '../podcasts/EpisodeList';
import { EpisodeOrderButton } from '../podcasts/EpisodeOrderButton';
import { formatLength } from '../podcasts/formatLength';
import { PodcastArt } from '../podcasts/PodcastArt';
import { ShowNotes } from '../podcasts/ShowNotes';
import { ShowSpeedButton } from '../podcasts/ShowSpeedButton';
import {
  useForgetPosition,
  useOpenPodcastPage,
  useRemoveDownloads,
  useSetAllPlayed,
  useSetEpisodeOrder,
} from '../podcasts/usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';
import { OverflowMenu } from '../shell/OverflowMenu';
import { TopBar } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';
import { useSearchMode } from '../shell/useSearchMode';
import { matchesSearch } from '../shell/useListSearch';
import { usePlayContext } from '../music/playContext';
import { findInCache } from '../shell/queryMemory';

/**
 * An audiobook (M46.22): Resume where listening goes on or start over, add it to the library or
 * download it, its speed, and its chapters, each with its progress and download, sorted and
 * filtered its own way (M46.48).
 */
export const AudiobookPage: FC = () => {
  const { t } = useTranslation();
  const find = useSearchMode(t('audiobooks.findChapter'));
  const { id } = useSearch({ from: '/audiobook' });
  const book = useAudiobook(id);
  const play = usePlayAudiobook();
  const keep = useKeepAudiobook();
  const download = useDownloadAudiobook();
  const removeDownloads = useRemoveDownloads();
  const setAll = useSetAllPlayed();
  const setOrder = useSetEpisodeOrder();
  const openPage = useOpenPodcastPage();
  const forget = useForgetPosition();
  const client = useQueryClient();
  const view = book.data;
  // A feed without its own picture shows the one the directory listed it with (M46.45).
  const artworkUrl =
    view?.artworkUrl ??
    (view
      ? findInCache(client, ['audiobooks', 'directory'], (record) =>
          record.feedUrl === view.feedUrl && typeof record.artworkUrl === 'string'
            ? record.artworkUrl
            : null,
        )
      : null);
  usePlayContext(
    view
      ? {
          kind: 'audiobook',
          name: view.title,
          subtitle: view.author,
          artworkUrl,
          to: '/audiobook',
          search: { id },
        }
      : null,
  );

  if (book.error && !view) {
    return (
      <section data-testid="audiobook-page">
        <SourceIssue onRetry={() => void book.refetch()}>{book.error.message}</SourceIssue>
      </section>
    );
  }
  if (!view) {
    return (
      <section data-testid="audiobook-page" className="grid place-items-center py-12">
        <LoaderCircle aria-label={t('audiobooks.loading')} className="text-muted animate-spin" />
      </section>
    );
  }
  const resume = view.resume;
  const started = view.finished > 0 || (resume?.positionMs ?? 0) > 0;
  const firstChapter = view.chapters.find((chapter) => chapter.number === 1) ?? view.chapters[0];
  const totalMs = view.totalMs ?? 0;
  const listenedMs = view.listenedMs ?? 0;
  const left = Math.max(0, totalMs - listenedMs);
  const share = totalMs > 0 ? Math.min(1, listenedMs / totalMs) : 0;
  const shown = view.chapters.filter((chapter) => matchesSearch(find.query, [chapter.title]));
  const filtered = view.order.filters.length > 0 || find.query.trim() !== '';
  const allDownloaded = view.chapterCount > 0 && view.downloaded === view.chapterCount;
  const downloadedCount = view.chapters.filter((chapter) => chapter.download).length;
  const error =
    play.error ??
    keep.error ??
    download.error ??
    removeDownloads.error ??
    setAll.error ??
    setOrder.error ??
    openPage.error;

  return (
    <section data-testid="audiobook-page" className="flex flex-col gap-5">
      <TopBar
        back
        fallback="/library"
        plainTitle
        title={t('audiobooks.title')}
        search={find.search}
        actions={
          <>
            {find.action}
            <EpisodeOrderButton
              order={view.order}
              custom={view.customOrder}
              book
              onChange={(order) => setOrder.mutate({ id: view.id, order })}
            />
            <PinButton
              target={{
                kind: 'audiobook',
                to: '/audiobook',
                search: { id: view.id },
                name: view.title,
                artworkUrl: artworkUrl ?? null,
              }}
            />
            <OverflowMenu
              items={[
                {
                  label: t('audiobooks.markAllFinished'),
                  icon: CheckCheck,
                  onSelect: () => setAll.mutate({ id: view.id, played: true }),
                },
                {
                  label: t('audiobooks.markAllUnfinished'),
                  icon: Check,
                  onSelect: () => setAll.mutate({ id: view.id, played: false }),
                },
                ...(downloadedCount > 0
                  ? [
                      {
                        label: t('podcasts.removeDownloads'),
                        icon: Trash2,
                        confirm: t('podcasts.removeDownloadsConfirm', { count: downloadedCount }),
                        onSelect: () => removeDownloads.mutate(view.id),
                      },
                    ]
                  : []),
                ...(view.link
                  ? [
                      {
                        label: t('audiobooks.openPage'),
                        icon: Globe,
                        onSelect: () => view.link && openPage.mutate(view.link),
                      },
                    ]
                  : []),
              ]}
            />
          </>
        }
      />
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        <PodcastArt url={artworkUrl} className="w-40 shrink-0 rounded-lg shadow-xl sm:w-48" />
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('audiobooks.one')}
          </span>
          <h1 className="text-center text-2xl font-black tracking-tight wrap-anywhere sm:text-left sm:text-4xl">
            {view.title}
          </h1>
          {view.author && <p className="text-muted text-sm">{view.author}</p>}
          <p className="text-muted text-xs">
            {[
              view.edition,
              t('audiobooks.chapters', { count: view.chapterCount }),
              totalMs > 0 ? formatLength(totalMs, t) : null,
              started && totalMs > 0 && left > 0
                ? t('podcasts.left', { time: formatLength(left, t) })
                : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </p>
          {started && (
            <span
              aria-hidden
              className="bg-foreground/10 block h-1 w-48 overflow-hidden rounded-full"
            >
              <span
                className="bg-primary block h-full origin-left"
                style={{ transform: `scaleX(${share})` }}
              />
            </span>
          )}
        </div>
      </header>
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          disabled={view.chapterCount === 0 || play.isPending}
          onClick={() =>
            play.mutate({
              id: view.id,
              chapterId: resume ? null : (firstChapter?.id ?? null),
            })
          }
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {!resume && view.chapterCount > 0
            ? t('audiobooks.playAgain')
            : started && resume
              ? t('audiobooks.resume', { chapter: resume.number })
              : t('audiobooks.play')}
        </button>
        {started && resume && firstChapter && (
          <button
            type="button"
            disabled={play.isPending || forget.isPending}
            // The book's very start: the first chapter's own place is forgotten first.
            onClick={() =>
              void forget
                .mutateAsync(firstChapter.id)
                .catch(() => undefined)
                .then(() => play.mutate({ id: view.id, chapterId: firstChapter.id }))
            }
            className="bg-surface flex items-center gap-2 rounded-full px-5 py-2.5"
          >
            <RotateCcw aria-hidden size={16} />
            {t('audiobooks.fromStart')}
          </button>
        )}
        <button
          type="button"
          aria-pressed={view.inLibrary}
          disabled={keep.isPending}
          onClick={() => keep.mutate({ id: view.id, keep: !view.inLibrary })}
          className="bg-surface flex items-center gap-2 rounded-full px-5 py-2.5 disabled:opacity-60"
        >
          {view.inLibrary ? <Check aria-hidden size={16} /> : <Plus aria-hidden size={16} />}
          {t(view.inLibrary ? 'audiobooks.inLibrary' : 'audiobooks.add')}
        </button>
        <button
          type="button"
          disabled={allDownloaded || view.downloading > 0 || download.isPending}
          onClick={() => download.mutate(view.id)}
          className="bg-surface flex items-center gap-2 rounded-full px-5 py-2.5 disabled:opacity-60"
        >
          {view.downloading > 0 ? (
            <LoaderCircle aria-hidden size={16} className="animate-spin" />
          ) : allDownloaded ? (
            <Check aria-hidden size={16} />
          ) : (
            <Download aria-hidden size={16} />
          )}
          {view.downloading > 0
            ? t('audiobooks.downloadingBook', {
                done: view.downloaded,
                count: view.chapterCount,
              })
            : allDownloaded
              ? t('audiobooks.downloadedBook')
              : t('audiobooks.downloadBook')}
        </button>
        <ShowSpeedButton id={view.id} speed={view.speed} />
      </div>
      {error && <SourceIssue>{error.message}</SourceIssue>}
      {view.description && <ShowNotes text={view.description} clamp />}
      <section aria-labelledby="audiobook-chapters" className="flex flex-col gap-2">
        <h2 id="audiobook-chapters" className="font-semibold">
          {filtered && shown.length !== view.chapterCount
            ? t('audiobooks.chaptersShown', { shown: shown.length, count: view.chapterCount })
            : t('audiobooks.chapters', { count: view.chapterCount })}
        </h2>
        {shown.length === 0 ? (
          <div className="flex flex-col items-start gap-2">
            <p className="text-muted text-sm">{t('audiobooks.noneShown')}</p>
            {view.order.filters.length > 0 && (
              <button
                type="button"
                onClick={() =>
                  setOrder.mutate({ id: view.id, order: { ...view.order, filters: [] } })
                }
                className="bg-surface rounded-full px-4 py-1.5 text-sm"
              >
                {t('audiobooks.showAll')}
              </button>
            )}
          </div>
        ) : (
          <EpisodeList episodes={shown}>
            {(chapter) => (
              <EpisodeItem
                key={chapter.id}
                episode={chapter}
                chapter
                tapToPlay
                onPlay={() => play.mutate({ id: view.id, chapterId: chapter.id })}
              />
            )}
          </EpisodeList>
        )}
      </section>
    </section>
  );
};
