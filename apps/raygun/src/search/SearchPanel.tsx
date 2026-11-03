import { useEffect, useRef } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AllResults } from './AllResults';
import { CatalogSearch } from './CatalogSearch';
import { LibraryMatches } from './LibraryMatches';
import { MusicResults } from './MusicResults';
import { pill, SEARCH_TYPES, VIDEO_VIEWS } from './searchTypes';
import type { MusicView, SearchType, SearchView, VideoView } from './searchTypes';
import { VideoResults } from './VideoResults';

/** Movies & series (M46.25): Relevant, Movies or Series. Video sources don't say what a result
 * is until it's opened, so their results stay under every pill, after the addons' rows. */
const VideoSearch: FC<{ query: string; view: VideoView; onView: (view: VideoView) => void }> = ({
  query,
  view,
  onView,
}) => {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col gap-5">
      <div role="group" aria-label={t('search.video.kind')} className="flex gap-2">
        {VIDEO_VIEWS.map((option) => (
          <button
            key={option}
            type="button"
            aria-pressed={view === option}
            onClick={() => onView(option)}
            className={pill(view === option)}
          >
            {t(`search.video.${option}`)}
          </button>
        ))}
      </div>
      <VideoResults
        query={query}
        kind={view === 'movies' ? 'movie' : view === 'series' ? 'series' : undefined}
      />
      <section aria-label={t('search.fromSources')} className="flex flex-col gap-3">
        <h2 className="font-semibold">{t('search.fromSources')}</h2>
        {view !== 'relevant' && <p className="text-muted text-sm">{t('search.video.anyKind')}</p>}
        <CatalogSearch key="video" medium="video" query={query} />
      </section>
    </div>
  );
};

/**
 * Browse → Search: everything at once, or one kind of media with its sources' filters and tags.
 * Manga, novel and video sources browse with filters alone, and podcasts show top shows, before
 * anything is typed. The query comes from Browse's search bar.
 */
export const SearchPanel: FC<{
  query: string;
  type: SearchType;
  onType: (type: SearchType, view?: SearchView) => void;
  /** The pill within Music or Movies & series (M46.25). */
  view?: SearchView;
}> = ({ query, type, onType, view }) => {
  const { t } = useTranslation();
  // The library needs something typed; the rest show what's popular or a hint until then.
  const needsQuery = type === 'library';
  const musicView = (view ?? 'relevant') as MusicView;
  const videoView: VideoView = view === 'movies' || view === 'series' ? view : 'relevant';
  const selected = useRef<HTMLButtonElement>(null);

  // A link can open a kind whose chip is off-screen (Library → Manga's "Find in Browse").
  useEffect(() => {
    // jsdom has no scrollIntoView.
    selected.current?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [type]);

  return (
    <div data-testid="search-page" className="flex flex-col gap-4">
      <div
        role="tablist"
        aria-label={t('search.typeLabel')}
        className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
      >
        {SEARCH_TYPES.map((option) => (
          <button
            key={option}
            ref={type === option ? selected : undefined}
            type="button"
            role="tab"
            aria-selected={type === option}
            onClick={() => onType(option)}
            className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${type === option ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
          >
            {t(`search.type.${option}`)}
          </button>
        ))}
      </div>
      {!query && needsQuery && <p className="text-muted text-sm">{t(`search.hintFor.${type}`)}</p>}
      {type === 'all' && <AllResults query={query} onType={onType} />}
      {type === 'music' && (
        <MusicResults query={query} view={musicView} onView={(next) => onType('music', next)} />
      )}
      {type === 'video' && (
        <VideoSearch query={query} view={videoView} onView={(next) => onType('video', next)} />
      )}
      {query && type === 'library' && <LibraryMatches query={query} />}
      {(type === 'manga' || type === 'novel') && (
        <CatalogSearch key={type} medium={type} query={query} />
      )}
    </div>
  );
};
