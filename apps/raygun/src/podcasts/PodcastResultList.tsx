import type { PodcastSearchResult } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { openContextMenu } from '../shell/contextMenuStore';
import { PodcastArt } from './PodcastArt';
import { showMenu } from './podcastMenus';
import { useOpenFeed } from './usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';

/**
 * Directory results; tapping one opens (and stores) its feed, then its page. `grid` shows them as
 * covers (M46.36: Search's Podcasts cards), the list otherwise.
 */
export const PodcastResultList: FC<{ results: PodcastSearchResult[]; grid?: boolean }> = ({
  results,
  grid = false,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const openFeed = useOpenFeed();
  const [opening, setOpening] = useState<string | null>(null);

  const open = (feedUrl: string) => {
    setOpening(feedUrl);
    openFeed.mutate(feedUrl, {
      onSuccess: (view) => void navigate({ to: '/podcast', search: { id: view.id } }),
      onSettled: () => setOpening(null),
    });
  };

  if (grid) {
    return (
      <div className="flex flex-col gap-2">
        <ul
          data-testid="podcast-grid"
          className="grid grid-cols-[repeat(auto-fill,minmax(6.5rem,1fr))] gap-2"
        >
          {results.map((result) => (
            <li
              key={result.feedUrl}
              onContextMenu={(event) =>
                openContextMenu(event, showMenu(result, t, { open: () => open(result.feedUrl) }))
              }
            >
              <button
                type="button"
                onClick={() => open(result.feedUrl)}
                disabled={opening !== null}
                title={result.author ? `${result.title} · ${result.author}` : result.title}
                className="hover:bg-surface relative flex w-full flex-col gap-1 rounded-lg p-1.5 text-left disabled:opacity-70"
              >
                <PodcastArt url={result.artworkUrl} className="w-full rounded-md" />
                <span className="line-clamp-2 text-xs font-medium">{result.title}</span>
                {opening === result.feedUrl && (
                  <LoaderCircle
                    aria-label={t('podcasts.opening')}
                    size={20}
                    className="absolute top-3 right-3 animate-spin"
                  />
                )}
              </button>
            </li>
          ))}
        </ul>
        {openFeed.error && <SourceIssue>{openFeed.error.message}</SourceIssue>}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <ul className="bg-surface flex flex-col rounded-xl">
        {results.map((result) => (
          <li
            key={result.feedUrl}
            onContextMenu={(event) =>
              openContextMenu(event, showMenu(result, t, { open: () => open(result.feedUrl) }))
            }
          >
            <button
              type="button"
              onClick={() => open(result.feedUrl)}
              disabled={opening !== null}
              className="flex w-full items-center gap-3 px-4 py-3 text-left disabled:opacity-70"
            >
              <PodcastArt url={result.artworkUrl} className="w-12 shrink-0 rounded-md" />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate font-medium">{result.title}</span>
                <span className="text-muted truncate text-xs">
                  {[result.author, result.subscribed ? t('podcasts.subscribed') : null]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
              {opening === result.feedUrl && (
                <LoaderCircle
                  aria-label={t('podcasts.opening')}
                  size={18}
                  className="animate-spin"
                />
              )}
            </button>
          </li>
        ))}
      </ul>
      {openFeed.error && <SourceIssue>{openFeed.error.message}</SourceIssue>}
    </div>
  );
};
