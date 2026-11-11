import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useHasProvider } from '../extensions/useExtensions';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { useDrawnCount } from '../shell/useDrawnCount';
import { NoSources } from './NoSources';

/** Shows drawn at first, then as many more at a time (`useDrawnCount`). */
const FIRST_SHOWS = 24;

/** The directories asked; those waiting for a key are counted in one line. */
const DIRECTORY_KINDS = ['podcast.directory'];

/** Podcasts: the directory's results, or its top shows before anything is typed (Music's
 * Podcasts pill since M46.25). Without a podcast directory, the way to one. */
export const PodcastSearch: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const directory = useHasProvider('podcast.directory');
  const found = usePodcastSearch(directory === false ? '' : query);
  const top = useTopPodcasts(query === '' && directory !== false);
  const search = query ? found : top;
  // Many shows: the first at once, the rest as each part is on the screen.
  const drawn = useDrawnCount(search.data?.length ?? 0, FIRST_SHOWS, FIRST_SHOWS, query.trim());
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
      {search.data && search.data.length > 0 && (
        <PodcastResultList results={search.data.slice(0, drawn)} />
      )}
      {!search.isPending && <NeedsSetupNote kinds={DIRECTORY_KINDS} />}
    </div>
  );
};
