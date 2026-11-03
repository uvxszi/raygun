import type { MangaSource, SourceSearchResult } from '@raygun/bindings';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { ChevronRight } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import type { Medium } from '../manga/medium';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { usePodcastSearch, useTopPodcasts } from '../podcasts/usePodcasts';
import { useMangaSources } from '../manga/useManga';
import { SourceRow } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { MusicResults } from './MusicResults';
import { VideoResults } from './VideoResults';
import type { SearchType, SearchView } from './searchTypes';
import { sourceSearchOptions } from './useCatalogSearch';
import { SourceIssue } from '../shell/SourceIssue';

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

/** Rows a section of Search → All shows. */
const PREVIEW_ROWS = 2;

/**
 * The first two sources with results (or popular titles), as short rows. Sources are asked in
 * order until two have something (M46.7), not all at once.
 */
const CatalogPreview: FC<{ medium: Medium; query: string }> = ({ medium, query }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  if (!sources.data) return <p className="text-muted text-sm">{t('search.searching')}</p>;
  if (sources.data.length === 0) {
    return <p className="text-muted text-sm">{t('search.noCatalogSources')}</p>;
  }
  return <PreviewRows key={query.trim()} medium={medium} query={query} sources={sources.data} />;
};

const PreviewRows: FC<{ medium: Medium; query: string; sources: MangaSource[] }> = ({
  medium,
  query,
  sources,
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  // Sources are asked in order, as many as could still fill the rows: answered ones with
  // results count, pending ones might (M46.7). Each answer re-renders and may ask the next.
  let asked = 0;
  let filled = 0;
  for (const source of sources) {
    if (filled >= PREVIEW_ROWS) break;
    asked += 1;
    const state = client.getQueryState<SourceSearchResult | null>(
      sourceSearchOptions(medium, source.id, query).queryKey,
    );
    const answered = state?.status === 'success' || state?.status === 'error';
    if (!answered || (state.data?.items.length ?? 0) > 0) filled += 1;
  }
  const results = useQueries({
    queries: sources.slice(0, asked).map((source) => sourceSearchOptions(medium, source.id, query)),
  });
  const settled = results.every((result) => !result.isPending);
  const found = results
    .map((result) => result.data)
    .filter((result) => result !== null && result !== undefined && result.items.length > 0);
  const more = found.length < PREVIEW_ROWS && asked < sources.length;

  if (found.length === 0) {
    return (
      <p className="text-muted text-sm">
        {settled && !more ? t('search.noResults', { query }) : t('search.searching')}
      </p>
    );
  }
  return (
    <div className="flex flex-col gap-4">
      {found.slice(0, PREVIEW_ROWS).map((result) => (
        <SourceRow key={result!.source.id} result={result!} medium={medium} limit={3} />
      ))}
    </div>
  );
};

const PodcastPreview: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const found = usePodcastSearch(query);
  const top = useTopPodcasts(query === '');
  const search = query ? found : top;
  if (search.error) {
    return <SourceIssue>{search.error.message}</SourceIssue>;
  }
  if (search.data?.length === 0) {
    return <p className="text-muted text-sm">{t('podcasts.noResults')}</p>;
  }
  return search.data ? <PodcastResultList results={search.data.slice(0, 3)} /> : null;
};

/**
 * Search → All: a little of everything, each section leading to its full results. Before
 * anything is typed it shows what's popular: addons' catalogs, top podcasts, sources' popular
 * titles.
 */
export const AllResults: FC<{
  query: string;
  onType: (type: SearchType, view?: SearchView) => void;
}> = ({ query, onType }) => {
  const { t } = useTranslation();
  const blank = query.trim() === '';
  return (
    <div data-testid="all-results" className="flex flex-col gap-8">
      {blank ? (
        <p className="text-muted text-sm">{t('search.popularHint')}</p>
      ) : (
        <>
          <Section title={t('search.inYourLibrary')} onMore={() => onType('library')}>
            <LibraryMatches query={query} limit={4} />
          </Section>
          <Section title={t('library.media.music')} onMore={() => onType('music')}>
            <MusicResults query={query} compact />
          </Section>
        </>
      )}
      <Section title={t('library.media.podcast')} onMore={() => onType('music', 'podcasts')}>
        <PodcastPreview query={query} />
      </Section>
      <Section title={t('search.type.video')} onMore={() => onType('video')}>
        <VideoResults query={query} limit={2} />
        <CatalogPreview medium="video" query={query} />
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
