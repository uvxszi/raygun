import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useHasProvider } from '../extensions/useExtensions';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { SourceIssue } from '../shell/SourceIssue';
import { NoSources } from './NoSources';

/** Podcasts: the directory's results, or its top shows before anything is typed (Music's
 * Podcasts pill since M46.25). Without a podcast directory, the way to one. */
export const PodcastSearch: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const directory = useHasProvider('podcast.directory');
  const found = usePodcastSearch(directory === false ? '' : query);
  const top = useTopPodcasts(query === '' && directory !== false);
  const search = query ? found : top;
  if (directory === false) {
    return (
      <div data-testid="podcast-search">
        <NoSources label={t('podcasts.noDirectory')} />
      </div>
    );
  }
  return (
    <div data-testid="podcast-search" className="flex flex-col gap-3">
      {!query && <h2 className="font-semibold">{t('podcasts.topShows')}</h2>}
      {search.error && <SourceIssue>{search.error.message}</SourceIssue>}
      {search.data?.length === 0 && query && (
        <p className="text-muted text-sm">{t('podcasts.noResults')}</p>
      )}
      {search.data && search.data.length > 0 && <PodcastResultList results={search.data} />}
    </div>
  );
};
