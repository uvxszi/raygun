import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Check, Clapperboard, Compass, Puzzle, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryTop } from '../library/LibraryTop';
import { EmptyState } from '../shell/EmptyState';
import { unwrapResult } from '../lib/commandResult';
import { ContinueWatchingRow } from './ContinueWatchingRow';
import { useContinueWatching, useRemoveContinue, useResumeVideo } from './useWatch';

/** Library → Videos: what you're watching, then everything watched or started, newest first. */
export const VideosLibraryView: FC = () => {
  const { t } = useTranslation();
  // Keeps the list current while the player reports progress.
  useContinueWatching();
  const history = useQuery({
    queryKey: ['watch', 'history'],
    queryFn: () => unwrapResult(commands.videoHistory()),
  });
  const resume = useResumeVideo();
  const remove = useRemoveContinue();

  return (
    <div data-testid="videos-library" className="flex flex-col gap-5">
      <LibraryTop
        media="video"
        menu={[
          { label: t('browse.title'), icon: Compass, to: '/browse' },
          { label: t('watch.addons'), icon: Puzzle, to: '/watch/addons' },
        ]}
      />
      <ContinueWatchingRow />
      {history.data?.length === 0 && (
        <EmptyState
          icon={Clapperboard}
          title={t('library.videos.emptyTitle')}
          body={t('library.videos.empty')}
          action={{ label: t('library.videos.openWatch'), icon: Compass, to: '/browse' }}
        />
      )}
      {(history.data?.length ?? 0) > 0 && (
        <h2 className="text-lg font-semibold">{t('library.videos.history')}</h2>
      )}
      <ul className="bg-surface flex flex-col rounded-xl">
        {history.data?.map((item) => {
          const duration = item.durationMs ?? 0;
          const position = item.positionMs ?? 0;
          return (
            <li
              key={item.key}
              aria-label={item.title}
              className="flex items-center gap-3 px-4 py-3"
            >
              <button
                type="button"
                onClick={() => resume.mutate(item.key)}
                className="flex min-w-0 flex-1 items-center gap-3 text-left"
              >
                {item.poster ? (
                  <img
                    src={item.poster}
                    alt=""
                    className="aspect-video w-20 shrink-0 rounded object-cover"
                  />
                ) : (
                  <span
                    aria-hidden
                    className="bg-background text-muted grid aspect-video w-20 shrink-0 place-items-center rounded"
                  >
                    <Clapperboard size={18} />
                  </span>
                )}
                <span className="flex min-w-0 flex-col gap-1">
                  <span className="truncate font-medium">{item.title}</span>
                  {item.watched ? (
                    <span className="text-muted flex items-center gap-1 text-xs">
                      <Check aria-hidden size={12} />
                      {t('library.videos.watched')}
                    </span>
                  ) : (
                    duration > 0 && (
                      <span
                        aria-hidden
                        className="bg-background block h-1 w-32 overflow-hidden rounded-full"
                      >
                        <span
                          className="bg-primary block h-full"
                          style={{ width: `${Math.min(100, (position / duration) * 100)}%` }}
                        />
                      </span>
                    )
                  )}
                </span>
              </button>
              <button
                type="button"
                aria-label={t('library.videos.remove', { title: item.title })}
                onClick={() => remove.mutate(item.key, { onSettled: () => void history.refetch() })}
                className="text-muted p-2"
              >
                <X aria-hidden size={16} />
              </button>
            </li>
          );
        })}
      </ul>
      {resume.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {resume.error.message}
        </p>
      )}
    </div>
  );
};
