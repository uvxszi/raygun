import { useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate, useRouter, useSearch } from '@tanstack/react-router';
import { ArrowLeft, ChevronLeft, ChevronRight, LoaderCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MANGA, saveProgress, useMangaChapter } from '../manga/useManga';

/** Progress is saved this long after scrolling stops, and right away on the last page. */
const SAVE_DELAY_MS = 800;

/** A vertical strip of a chapter's pages (M24; the paged reader comes in M27). */
export const MangaReaderPage: FC = () => {
  const { t } = useTranslation();
  const { title, chapter } = useSearch({ from: '/manga/read' });
  const navigate = useNavigate({ from: '/manga/read' });
  const router = useRouter();
  const queryClient = useQueryClient();
  const loaded = useMangaChapter(title, chapter);
  const [current, setCurrent] = useState(0);
  const [chrome, setChrome] = useState(true);
  const pages = useRef<(HTMLImageElement | null)[]>([]);
  const data = loaded.data;
  const total = data?.pages.length ?? 0;

  // Which page is on screen: the one most visible.
  useEffect(() => {
    if (!data || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            setCurrent(Number((entry.target as HTMLElement).dataset.index ?? 0));
          }
        }
      },
      { threshold: 0.5 },
    );
    for (const page of pages.current) {
      if (page) observer.observe(page);
    }
    return () => observer.disconnect();
  }, [data]);

  // Resume where this chapter was left.
  useEffect(() => {
    const resume = data?.chapter.lastPage;
    if (resume) pages.current[resume]?.scrollIntoView?.();
  }, [data]);

  useEffect(() => {
    if (!data || total === 0) return;
    const save = () => void saveProgress(data.chapter.id, current, total);
    if (current >= total - 1) {
      save();
      return;
    }
    const timer = setTimeout(save, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [current, data, total]);

  // The title page and library show read state; refresh them when leaving the reader.
  useEffect(
    () => () => {
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] });
      void queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] });
    },
    [queryClient],
  );

  const goTo = (chapterId: string | null) => {
    if (!chapterId) return;
    setCurrent(0);
    void navigate({ search: { title, chapter: chapterId } });
  };

  return (
    <section data-testid="manga-reader" className="-mx-4 flex flex-col bg-black lg:-mx-10">
      {chrome && (
        <header className="bg-background/90 fixed inset-x-0 top-0 z-30 flex items-center gap-2 px-2 pt-[env(safe-area-inset-top)] backdrop-blur">
          <button
            type="button"
            aria-label={t('manga.closeReader')}
            onClick={() => router.history.back()}
            className="grid size-11 place-items-center"
          >
            <ArrowLeft aria-hidden size={22} />
          </button>
          <span className="flex min-w-0 flex-col py-2">
            <span className="truncate text-sm font-semibold">{data?.title}</span>
            <span className="text-muted truncate text-xs">{data?.chapter.name}</span>
          </span>
        </header>
      )}
      {loaded.error && (
        <p role="alert" className="text-primary p-6 pt-20 text-sm wrap-anywhere">
          {loaded.error.message}
        </p>
      )}
      {!data && !loaded.error && (
        <LoaderCircle
          aria-label={t('manga.loading')}
          className="text-muted m-auto mt-32 animate-spin"
        />
      )}
      {data && (
        <>
          <button
            type="button"
            aria-label={t('manga.toggleControls')}
            onClick={() => setChrome((shown) => !shown)}
            className="flex flex-col"
          >
            {data.pages.map((url, index) => (
              <img
                key={url}
                ref={(element) => {
                  pages.current[index] = element;
                }}
                data-index={index}
                data-testid="manga-page"
                src={url}
                alt={t('manga.pageAlt', { page: index + 1 })}
                loading={index < 3 ? 'eager' : 'lazy'}
                decoding="async"
                className="min-h-[60vh] w-full bg-neutral-900 object-contain"
                onLoad={(event) => event.currentTarget.classList.remove('min-h-[60vh]')}
              />
            ))}
          </button>
          <div className="text-foreground flex flex-col items-center gap-3 px-4 py-10">
            <p className="text-muted text-sm">{t('manga.endOf', { name: data.chapter.name })}</p>
            <div className="flex gap-3">
              {data.previousChapterId && (
                <button
                  type="button"
                  onClick={() => goTo(data.previousChapterId)}
                  className="bg-surface flex items-center gap-1 rounded-full px-4 py-2 text-sm"
                >
                  <ChevronLeft aria-hidden size={16} />
                  {t('manga.previousChapter')}
                </button>
              )}
              {data.nextChapterId ? (
                <button
                  type="button"
                  onClick={() => goTo(data.nextChapterId)}
                  className="bg-primary text-background flex items-center gap-1 rounded-full px-5 py-2 text-sm font-semibold"
                >
                  {t('manga.nextChapter')}
                  <ChevronRight aria-hidden size={16} />
                </button>
              ) : (
                <Link
                  to="/library"
                  search={{ media: 'manga' }}
                  className="bg-surface rounded-full px-4 py-2 text-sm"
                >
                  {t('manga.backToLibrary')}
                </Link>
              )}
            </div>
          </div>
          {chrome && total > 0 && (
            <p
              data-testid="page-indicator"
              className="bg-background/80 fixed right-3 bottom-[calc(0.75rem+env(safe-area-inset-bottom))] z-30 rounded-full px-3 py-1 text-xs tabular-nums"
            >
              {t('manga.pageOf', { page: Math.min(current + 1, total), total })}
            </p>
          )}
        </>
      )}
    </section>
  );
};
