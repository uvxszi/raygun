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

/** Directory results; tapping one opens (and stores) its feed, then its page. */
export const PodcastResultList: FC<{ results: PodcastSearchResult[] }> = ({ results }) => {
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
