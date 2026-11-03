import { useSearch } from '@tanstack/react-router';
import { Check, LoaderCircle, Play, Plus, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useKeepAudiobook, useAudiobook, usePlayAudiobook } from '../audiobooks/useAudiobooks';
import { EpisodeItem } from '../podcasts/EpisodeItem';
import { formatLength } from '../podcasts/formatLength';
import { PodcastArt } from '../podcasts/PodcastArt';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';
import { PinButton } from '../shell/PinButton';
import { useSearchMode } from '../shell/useSearchMode';
import { matchesSearch } from '../shell/useListSearch';
import { usePlayContext } from '../music/playContext';

/**
 * An audiobook (M46.22): Resume where you left off or start over, add it to the library, and its
 * chapters in order, each with its progress and download.
 */
export const AudiobookPage: FC = () => {
  const { t } = useTranslation();
  const find = useSearchMode(t('audiobooks.findChapter'));
  const { id } = useSearch({ from: '/audiobook' });
  const book = useAudiobook(id);
  const play = usePlayAudiobook();
  const keep = useKeepAudiobook();
  const [expanded, setExpanded] = useState(false);
  const view = book.data;
  usePlayContext(
    view
      ? {
          kind: 'audiobook',
          name: view.title,
          subtitle: view.author,
          artworkUrl: view.artworkUrl,
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
  const total = view.chapters.reduce((sum, chapter) => sum + (chapter.durationMs ?? 0), 0);
  const next = view.chapters.find((chapter) => !chapter.played);
  const started = view.chapters.some((chapter) => chapter.played || (chapter.positionMs ?? 0) > 0);

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
            <PinButton
              target={{
                kind: 'audiobook',
                to: '/audiobook',
                search: { id: view.id },
                name: view.title,
                artworkUrl: view.artworkUrl ?? null,
              }}
            />{' '}
          </>
        }
      />
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        <PodcastArt url={view.artworkUrl} className="w-40 shrink-0 rounded-lg shadow-xl sm:w-48" />
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('audiobooks.title')}
          </span>
          <h1 className="text-center text-2xl font-black tracking-tight wrap-anywhere sm:text-left sm:text-4xl">
            {view.title}
          </h1>
          {view.author && <p className="text-muted text-sm">{view.author}</p>}
          <p className="text-muted text-xs">
            {[
              t('audiobooks.chapters', { count: view.chapters.length }),
              total > 0 ? formatLength(total, t) : null,
            ]
              .filter(Boolean)
              .join(' · ')}
          </p>
        </div>
      </header>
      <div className="flex flex-wrap gap-3">
        <button
          type="button"
          disabled={view.chapters.length === 0 || play.isPending}
          onClick={() => play.mutate({ id: view.id, chapterId: null })}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {started && next
            ? t('audiobooks.resume', { chapter: view.chapters.indexOf(next) + 1 })
            : t('audiobooks.play')}
        </button>
        {started && (
          <button
            type="button"
            disabled={play.isPending}
            onClick={() => play.mutate({ id: view.id, chapterId: view.chapters[0]?.id ?? null })}
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
      </div>
      {(play.error ?? keep.error) && (
        <SourceIssue>{(play.error ?? keep.error)?.message}</SourceIssue>
      )}
      {view.description && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          aria-expanded={expanded}
          className={`text-muted text-left text-sm whitespace-pre-line wrap-anywhere ${expanded ? '' : 'line-clamp-3'}`}
        >
          {view.description}
        </button>
      )}
      <section aria-labelledby="audiobook-chapters" className="flex flex-col gap-2">
        <h2 id="audiobook-chapters" className="font-semibold">
          {t('audiobooks.chapters', { count: view.chapters.length })}
        </h2>
        <ul className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3">
          {view.chapters
            .filter((chapter) => matchesSearch(find.query, [chapter.title]))
            .map((chapter) => (
              <EpisodeItem
                key={chapter.id}
                episode={chapter}
                onPlay={() => play.mutate({ id: view.id, chapterId: chapter.id })}
              />
            ))}
        </ul>
      </section>
    </section>
  );
};
