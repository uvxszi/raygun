import type { MangaSource, RecentSearch } from '@raygun/bindings';
import { ChevronRight } from 'lucide-react';
import { Fragment, useEffect, useRef } from 'react';
import type { FC, MouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { AllResults } from './AllResults';
import { CatalogSearch, LazySourceRow, RankedResults } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { MusicResults } from './MusicResults';
import { useMangaSources } from '../manga/useManga';
import { RecentSearches } from './RecentSearches';
import { keepSource, useSearchFilter } from './searchFilter';
import { SearchFilterToggles } from './SearchFilterToggles';
import { kindOf, useRememberSearch } from './useRecentSearches';
import { useSearchStream } from './searchStream';
import { answeredFirst, failingLast, useSourceHealth } from './sourceHealth';
import { useSourceAnswers } from './useCatalogSearch';
import { pill, SEARCH_TYPES, VIDEO_VIEWS, viewIn } from './searchTypes';
import type { MusicView, SearchType, SearchView, VideoView } from './searchTypes';
import { VideoResults } from './VideoResults';
import { SelectionScope } from '../select/Selectable';
import { useSettings } from '../settings/useSettings';
import { useDragScroll } from '../shell/useDragScroll';
import { usePressedInView } from '../shell/usePressedInView';
import { usePlayContext } from '../music/playContext';
import { YouTubeLinkCard } from '../youtube/YouTubeLinkCard';
import { useYouTubeLink } from '../youtube/useYtDlp';

/** Video sources' best answers Relevant shows before See all. */
const RELEVANT_TITLES = 12;

const Section: FC<{ title: string; onMore?: () => void; children: ReactNode }> = ({
  title,
  onMore,
  children,
}) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="font-semibold">{title}</h2>
        {onMore && (
          <button
            type="button"
            onClick={onMore}
            className="text-primary flex items-center gap-0.5 text-sm"
          >
            {t('search.seeAll')}
            <ChevronRight aria-hidden size={16} />
          </button>
        )}
      </div>
      {children}
    </section>
  );
};

/**
 * Videos (M46.25): Relevant is the addons' movies and series, the video sources' best answers
 * and a few from each source with a pill of its own, each leading to its pill; Movies and Series
 * narrow the addons' rows; Sources searches the sources, chosen with Custom sources. Video
 * sources don't say what a result is until it's opened, so their answers stay under Movies and
 * Series too.
 */
const VideoSearch: FC<{
  query: string;
  view: VideoView;
  onView: (view: VideoView) => void;
  pills: MangaSource[];
  onPill: (id: string) => void;
}> = ({ query, view, onView, pills, onPill }) => {
  const { t } = useTranslation();
  const listed = useMangaSources('video');
  const failures = useSourceHealth((state) => state.failures);
  const failingAtEnd = useSettings().data?.failingSourcesLast ?? true;
  const filter = useSearchFilter();
  const sources = failingLast(
    (listed.data ?? []).filter(
      (source) => source.enabled && !source.searchPill && keepSource(filter, source),
    ),
    failures,
    failingAtEnd,
  );
  // The pills' rows: those with titles first, then those with none, then failures.
  const answers = useSourceAnswers(
    'video',
    pills.map((source) => source.id),
    query,
  );
  const pillRows = answeredFirst(pills, (source) => answers(source.id));
  return (
    <div className="flex flex-col gap-5">
      {view === 'sources' && (
        // Every video source, the ones with their own pill too (M46.40).
        <CatalogSearch key="video-sources" medium="video" query={query} withPills />
      )}
      {view === 'relevant' && (
        <>
          <VideoResults query={query} />
          {sources.length > 0 && (
            <Section title={t('search.fromSources')} onMore={() => onView('sources')}>
              <RankedResults
                key={query.trim()}
                sources={sources}
                medium="video"
                query={query}
                limit={RELEVANT_TITLES}
              />
            </Section>
          )}
          {pillRows.map((source) => (
            <LazySourceRow
              key={`${source.id}:${query.trim()}`}
              source={source}
              medium="video"
              query={query}
              onMore={() => onPill(source.id)}
            />
          ))}
        </>
      )}
      {(view === 'movies' || view === 'series') && (
        <>
          <VideoResults query={query} kind={view === 'movies' ? 'movie' : 'series'} />
          <Section title={t('search.fromSources')}>
            <CatalogSearch key="video" medium="video" query={query} />
          </Section>
        </>
      )}
    </div>
  );
};

/** Videos' own pills: its views, then each video site that brings a pill (YouTube, PeerTube…). */
const VideoPillRow: FC<{
  view: VideoView;
  site: string | null;
  sites: MangaSource[];
  onView: (view: VideoView) => void;
  onSite: (id: string) => void;
}> = ({ view, site, sites, onView, onSite }) => {
  const { t } = useTranslation();
  const rowRef = useRef<HTMLDivElement>(null);
  const row = useDragScroll(rowRef);
  usePressedInView(rowRef, site ?? view);
  return (
    <div
      ref={row}
      role="group"
      aria-label={t('search.video.kind')}
      className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
    >
      {VIDEO_VIEWS.map((option) => (
        <button
          key={option}
          type="button"
          aria-pressed={site === null && view === option}
          onClick={() => onView(option)}
          className={`shrink-0 ${pill(site === null && view === option)}`}
        >
          {t(`search.video.${option}`)}
        </button>
      ))}
      {sites.map((source) => (
        <button
          key={source.id}
          type="button"
          aria-pressed={site === source.id}
          onClick={() => onSite(source.id)}
          className={`shrink-0 ${pill(site === source.id)}`}
        >
          {source.searchPill}
        </button>
      ))}
    </div>
  );
};

/** Whether a click opened or played something among the results (not a pill or a chip). */
const opensResult = (event: MouseEvent) => {
  const target = (event.target as Element | null)?.closest('a, button');
  return Boolean(target) && !target?.closest('[role="group"], [role="tablist"]');
};

/**
 * Browse → Search: everything at once, or one kind of media with its sources' filters and tags.
 * Before anything is typed it shows the searches made lately, then what's popular: sources'
 * popular titles, top podcasts, popular and new audiobooks. The query comes from Browse's search
 * bar. Each source's answer shows as it comes (`useSearchStream`); a search is remembered when
 * something it found is opened, or when Enter is pressed in the bar.
 */
export const SearchPanel: FC<{
  query: string;
  type: SearchType;
  onType: (type: SearchType, view?: SearchView, pill?: string) => void;
  /** Searches again for a kept search. */
  onSearch: (query: string, type: SearchType) => void;
  /** The pill within Music or Movies & series (M46.25). */
  view?: SearchView;
  /** A video source's own pill (M46.35): its id. */
  pill?: string;
}> = ({ query, type, onType, onSearch, view, pill }) => {
  const { t } = useTranslation();
  useSearchStream();
  const remember = useRememberSearch();
  // Video sources that bring their own pill (YouTube, PeerTube…), enabled ones only.
  const videoSources = useMangaSources('video');
  const filter = useSearchFilter();
  const pills = (videoSources.data ?? []).filter(
    (source) => source.searchPill && source.enabled && keepSource(filter, source),
  );
  const pillSource = type === 'video' ? pills.find((source) => source.id === pill) : undefined;
  const blank = query.trim() === '';
  // A YouTube address is answered by its card; the sources' matches for the address's words
  // would only be noise under it.
  const youtubeLink = useYouTubeLink(query);
  // What plays from here shows "Playing from" this search.
  usePlayContext(
    blank
      ? null
      : {
          kind: 'search',
          name: query.trim(),
          subtitle: null,
          artworkUrl: null,
          to: '/browse',
          search: {
            tab: 'search',
            type,
            q: query,
            ...(view ? { view } : {}),
            ...(pill ? { pill } : {}),
          },
        },
  );
  const musicView = viewIn<MusicView>('music', view);
  const videoView = viewIn<VideoView>('video', view);
  const selected = useRef<HTMLButtonElement>(null);
  const typeRow = useDragScroll<HTMLDivElement>();

  // A link can open a kind whose chip is off-screen (Library → Manga's "Find in Browse").
  useEffect(() => {
    // jsdom has no scrollIntoView.
    selected.current?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [type, pill]);

  const typePill = (option: SearchType) => {
    const current = type === option && !(option === 'video' && pillSource);
    return (
      <button
        key={option}
        ref={current ? selected : undefined}
        type="button"
        role="tab"
        aria-selected={current}
        onClick={() => onType(option)}
        className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${current ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
      >
        {t(`search.type.${option}`)}
      </button>
    );
  };

  return (
    <div data-testid="search-page" className="flex flex-col gap-4">
      {/* Holding a result (or Ctrl/Shift-click) picks it; the bar acts on all picked (M46.50). */}
      <SelectionScope>
        {/* The kinds, then Search's "No 18+" and "No explicit" beside them (below on phones). */}
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          <div
            ref={typeRow}
            role="tablist"
            aria-label={t('search.typeLabel')}
            className="-mx-4 flex min-w-0 flex-1 gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
          >
            {SEARCH_TYPES.map((option) => (
              <Fragment key={option}>{typePill(option)}</Fragment>
            ))}
          </div>
          <SearchFilterToggles />
        </div>
        {/* A YouTube address plays or downloads as it is (M46.47). */}
        <YouTubeLinkCard query={query} />
        {blank && (
          <RecentSearches
            type={type}
            onPick={(search: RecentSearch) => onSearch(search.query, kindOf(search))}
          />
        )}
        {blank && type === 'library' && (
          <p className="text-muted text-sm">{t('search.hintFor.library')}</p>
        )}
        {/* Opening or playing something found remembers the search. */}
        {!youtubeLink.data && (
          <div
            className="flex flex-col gap-4"
            onClickCapture={(event) => {
              if (!blank && opensResult(event)) remember(query, type);
            }}
          >
            {type === 'all' && <AllResults query={query} onType={onType} />}
            {type === 'music' && (
              <MusicResults
                query={query}
                view={musicView}
                onView={(next) => onType('music', next)}
              />
            )}
            {type === 'video' && (
              <VideoPillRow
                view={videoView}
                site={pillSource?.id ?? null}
                sites={pills}
                onView={(next) => onType('video', next)}
                onSite={(id) => onType('video', undefined, id)}
              />
            )}
            {type === 'video' && pillSource && (
              <CatalogSearch
                key={pillSource.id}
                medium="video"
                query={query}
                only={pillSource.id}
              />
            )}
            {type === 'video' && !pillSource && (
              <VideoSearch
                query={query}
                view={videoView}
                onView={(next) => onType('video', next)}
                pills={pills}
                onPill={(id) => onType('video', undefined, id)}
              />
            )}
            {!blank && type === 'library' && <LibraryMatches query={query} />}
            {(type === 'manga' || type === 'novel') && (
              <CatalogSearch
                key={type}
                medium={type}
                query={query}
                onLibrary={() => onType('library')}
              />
            )}
          </div>
        )}
      </SelectionScope>
    </div>
  );
};
