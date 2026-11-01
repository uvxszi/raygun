import { ChevronRight } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch } from '../podcasts/usePodcasts';
import { SourceRow } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { MusicResults } from './MusicResults';
import type { SearchType } from './searchTypes';
import { useGlobalSearch } from './useCatalogSearch';

const Section: FC<{
  title: string;
  onMore: () => void;
  children: ReactNode;
}> = ({ title, onMore, children }) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="font-semibold">{title}</h2>
        <button
          type="button"
          onClick={onMore}
          className="text-primary flex items-center gap-0.5 text-sm"
        >
          {t('search.seeAll')}
          <ChevronRight aria-hidden size={16} />
        </button>
      </div>
      {children}
    </section>
  );
};

/** The two sources with the most results, as short rows. */
const CatalogPreview: FC<{ medium: Medium; query: string }> = ({ medium, query }) => {
  const { t } = useTranslation();
  const search = useGlobalSearch(medium, query);
  const withResults = (search.data ?? []).filter((result) => result.items.length > 0).slice(0, 2);
  if (search.isFetching && !search.data) {
    return <p className="text-muted text-sm">{t('search.searching')}</p>;
  }
  if (search.data && withResults.length === 0) {
    return (
      <p className="text-muted text-sm">
        {search.data.length === 0 ? t('search.noCatalogSources') : t('search.noResults', { query })}
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-4">
      {withResults.map((result) => (
        <SourceRow key={result.source.id} result={result} medium={medium} limit={3} />
      ))}
    </div>
  );
};

const PodcastPreview: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const search = usePodcastSearch(query);
  if (search.error) {
    return (
      <p role="alert" className="text-primary text-sm wrap-anywhere">
        {search.error.message}
      </p>
    );
  }
  if (search.data?.length === 0) {
    return <p className="text-muted text-sm">{t('podcasts.noResults')}</p>;
  }
  return search.data ? <PodcastResultList results={search.data.slice(0, 3)} /> : null;
};

/** Search → All: a little of everything, each section leading to its full results. */
export const AllResults: FC<{ query: string; onType: (type: SearchType) => void }> = ({
  query,
  onType,
}) => {
  const { t } = useTranslation();
  return (
    <div data-testid="all-results" className="flex flex-col gap-8">
      <Section title={t('search.inYourLibrary')} onMore={() => onType('library')}>
        <LibraryMatches query={query} limit={4} />
      </Section>
      <Section title={t('library.media.music')} onMore={() => onType('music')}>
        <MusicResults query={query} compact />
      </Section>
      <Section title={t('library.media.podcast')} onMore={() => onType('podcast')}>
        <PodcastPreview query={query} />
      </Section>
      <Section title={t('library.media.manga')} onMore={() => onType('manga')}>
        <CatalogPreview medium="manga" query={query} />
      </Section>
      <Section title={t('library.media.novel')} onMore={() => onType('novel')}>
        <CatalogPreview medium="novel" query={query} />
      </Section>
    </div>
  );
};
