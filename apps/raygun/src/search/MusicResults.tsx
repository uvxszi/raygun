import type { LibraryTrack, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { AudiobookSearch } from './AudiobookSearch';
import { FailedSources } from './FailedSources';
import { NoSources } from './NoSources';
import { AlbumFinds, ArtistFinds, Asking, BestSongs } from './MusicFinds';
import { PlaylistFinds } from './PlaylistFinds';
import { OnlineTrackRow } from './OnlineTrackRow';
import { PodcastSearch } from './PodcastSearch';
import { RelevantMusic } from './RelevantMusic';
import { useSeenSongs } from './seenBefore';
import { answeredFirst, failingLast, useSourceHealth } from './sourceHealth';
import { MUSIC_VIEWS, pill } from './searchTypes';
import type { MusicView } from './searchTypes';
import { SourceMenu } from './SourceMenu';
import { playResults, useOnlineSearch } from './useOnlineSearch';
import { AdultMark } from '../shell/AdultMark';
import { SourceIssue } from '../shell/SourceIssue';
import { useSettings } from '../settings/useSettings';

type ProviderResultsProps = { result: ProviderSearchResult; ambiguous: boolean; limit?: number };

const ProviderResults: FC<ProviderResultsProps> = ({ result, ambiguous, limit }) => {
  const { t } = useTranslation();
  const tracks = limit ? result.tracks.slice(0, limit) : result.tracks;
  return (
    <section data-testid="provider-results" className="flex flex-col gap-1">
      <h3 className="text-muted flex min-w-0 items-center gap-1 text-sm font-medium">
        <span className="truncate">
          {result.provider.name}
          {/* Two sources with one name (a Nuclear plugin beside Raygun's own) need telling apart. */}
          {ambiguous && <span className="font-normal"> · {result.provider.extensionId}</span>}
        </span>
        {result.provider.adult && <AdultMark />}
        <SourceMenu
          sourceId={result.provider.id}
          extensionId={result.provider.extensionId}
          name={result.provider.name}
        />
      </h3>
      {result.error ? (
        <SourceIssue compact source={result.provider.name}>
          {result.error}
        </SourceIssue>
      ) : result.tracks.length === 0 ? (
        <p className="text-muted text-sm">{t('search.providerEmpty')}</p>
      ) : (
        <ul className="flex flex-col">
          {tracks.map((track, index) => (
            <li key={`${track.externalId ?? track.title}-${index}`}>
              <OnlineTrackRow
                track={track}
                onPlay={(tapped) => void playResults(result.tracks, tapped)}
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

/** Local tracks matching the query; tapping one plays the library from there. */
const LibraryResults: FC<{ tracks: LibraryTrack[] }> = ({ tracks }) => {
  const { t } = useTranslation();
  return (
    <section data-testid="library-results" className="flex flex-col gap-1">
      <h3 className="text-muted text-sm font-medium">{t('search.yourLibrary')}</h3>
      <ul className="flex flex-col">
        {tracks.map((track) => (
          <li key={track.id}>
            <TrackRow
              track={track}
              onPlay={() =>
                void playFromLibrary({
                  selection: { kind: 'allTracks' },
                  startTrackId: track.id,
                  shuffle: false,
                })
              }
            />
          </li>
        ))}
      </ul>
    </section>
  );
};

/** Songs several providers found, merged in Rust. */
const TopResults: FC<{ tracks: OnlineTrack[] }> = ({ tracks }) => {
  const { t } = useTranslation();
  return (
    <section data-testid="top-results" className="flex flex-col gap-1">
      <h3 className="text-muted text-sm font-medium">{t('search.topResults')}</h3>
      <ul className="flex flex-col">
        {tracks.map((track, index) => (
          <li key={`${track.title}-${index}`}>
            <OnlineTrackRow track={track} onPlay={(tapped) => void playResults(tracks, tapped)} />
          </li>
        ))}
      </ul>
    </section>
  );
};

const GROUPED_KEY = 'raygun.search.grouped.music';

const readGrouped = () => {
  try {
    return localStorage.getItem(GROUPED_KEY) === '1';
  } catch {
    return false;
  }
};

/** Remembers which music sources failed, so they go last next time (M46.9). */
const useMusicHealth = (results: ProviderSearchResult[]) => {
  const { failed, worked } = useSourceHealth();
  const report = results.map((result) => `${result.provider.id}:${result.error ? 1 : 0}`).join();
  useEffect(() => {
    for (const result of results) {
      if (result.error) failed(result.provider.id);
      else worked(result.provider.id);
    }
    // Reported once per change of the sources' answers.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [report]);
};

/**
 * Music results (M46.9): Relevant (M46.25: a bit of everything), songs, artists, albums,
 * playlists, podcasts or audiobooks. Songs are your library's, then the best matches from every
 * source (each naming its source), or each source's own list. `compact` keeps it short for the
 * All view (top results, else the first provider's). The pill lives in the address (`view`).
 */
export const MusicResults: FC<{
  query: string;
  compact?: boolean;
  view?: MusicView;
  onView?: (view: MusicView) => void;
}> = ({ query, compact, view = 'relevant', onView = () => undefined }) => {
  const { t } = useTranslation();
  const setView = onView;
  const [grouped, setGroupedState] = useState(readGrouped);
  const setGrouped = (next: boolean) => {
    setGroupedState(next);
    try {
      localStorage.setItem(GROUPED_KEY, next ? '1' : '0');
    } catch {
      // A per-device convenience.
    }
  };
  const search = useOnlineSearch(query);
  // Songs already seen show while the sources are asked (M46.47).
  const seen = useSeenSongs(query);
  const failures = useSourceHealth((state) => state.failures);
  const failingAtEnd = useSettings().data?.failingSourcesLast ?? true;
  const answered = search.data?.providers ?? [];
  useMusicHealth(answered);
  // Sources with songs lead, then those that found none, then failures; and those that failed
  // lately go last among each, as they do in every source list (M46.9).
  const results = answeredFirst(
    failingLast(
      answered.map((result) => ({ ...result, id: result.provider.id })),
      failures,
      failingAtEnd,
    ),
    (result) => (result.error ? 'failed' : result.tracks.length > 0 ? 'results' : 'empty'),
  );
  const library = search.data?.library ?? [];
  const top = search.data?.top ?? [];
  const names = results.map((result) => result.provider.name);
  const working = results.filter((result) => !result.error);
  const failed = results
    .filter((result) => result.error)
    .map((result) => ({ name: result.provider.name, error: result.error ?? '' }));
  const asking = search.isFetching;
  const nothingFound =
    !asking &&
    library.length === 0 &&
    results.length > 0 &&
    working.every((result) => result.tracks.length === 0);

  if (compact && search.error) {
    return <SourceIssue>{search.error.message}</SourceIssue>;
  }
  if (compact) {
    const first = working.find((result) => result.tracks.length > 0);
    if (top.length > 0) return <TopResults tracks={top.slice(0, 5)} />;
    if (first) return <ProviderResults result={first} ambiguous={false} limit={5} />;
    return search.isFetching ? (
      <Asking progress={search.progress} />
    ) : (
      <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
    );
  }
  const blank = query.trim() === '';
  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center gap-2">
        <div
          role="group"
          aria-label={t('search.music.kind')}
          className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {MUSIC_VIEWS.map((option) => (
            <button
              key={option}
              type="button"
              aria-pressed={view === option}
              onClick={() => setView(option)}
              className={pill(view === option)}
            >
              {t(`search.music.${option}`)}
            </button>
          ))}
        </div>
        {view === 'songs' && results.length > 1 && (
          <div
            role="group"
            aria-label={t('search.arrange')}
            className="bg-surface ml-auto flex rounded-full p-0.5 text-sm"
          >
            {([false, true] as const).map((byGroup) => (
              <button
                key={String(byGroup)}
                type="button"
                aria-pressed={grouped === byGroup}
                onClick={() => setGrouped(byGroup)}
                className={`rounded-full px-3 py-1 ${grouped === byGroup ? 'bg-primary/20 text-primary font-semibold' : 'text-muted'}`}
              >
                {t(byGroup ? 'search.bySource' : 'search.byResult')}
              </button>
            ))}
          </div>
        )}
      </div>
      {view === 'podcasts' && <PodcastSearch query={query} />}
      {view === 'audiobooks' && <AudiobookSearch query={query} />}
      {blank && view !== 'podcasts' && view !== 'audiobooks' && (
        <p className="text-muted text-sm">{t(`search.typeFor.${view}`)}</p>
      )}
      {!blank && view === 'relevant' && <RelevantMusic query={query} onView={setView} />}
      {!blank && view === 'artists' && <ArtistFinds query={query} />}
      {!blank && view === 'albums' && <AlbumFinds query={query} />}
      {!blank && view === 'playlists' && <PlaylistFinds query={query} />}
      {!blank && view === 'songs' && search.error && (
        <SourceIssue>{search.error.message}</SourceIssue>
      )}
      {!blank && view === 'songs' && !search.error && (
        <>
          {search.isSuccess && !asking && results.length === 0 && library.length === 0 && (
            <NoSources label={t('search.noSources')} />
          )}
          {library.length > 0 && <LibraryResults tracks={library} />}
          {asking && <Asking progress={search.progress} />}
          {nothingFound ? (
            <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
          ) : grouped ? (
            <>
              {!search.data && seen.length > 0 && (
                <BestSongs query={query} top={seen} results={[]} />
              )}
              {top.length > 0 && <TopResults tracks={top} />}
              {results.map((result) => (
                <ProviderResults
                  key={result.provider.id}
                  result={result}
                  ambiguous={names.filter((name) => name === result.provider.name).length > 1}
                />
              ))}
            </>
          ) : (
            <>
              {/* Songs seen before show in the same list until the sources answer. */}
              {(working.length > 0 || (!search.data && seen.length > 0)) && (
                <BestSongs
                  query={query}
                  top={search.data ? top : seen}
                  results={search.data ? working : []}
                />
              )}
              <FailedSources failed={failed} />
            </>
          )}
        </>
      )}
    </div>
  );
};
