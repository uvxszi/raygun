import type { LibraryTrack, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { AlbumFinds, ArtistFinds, BestSongs } from './MusicFinds';
import { PlaylistFinds } from './PlaylistFinds';
import { OnlineTrackRow } from './OnlineTrackRow';
import { PodcastSearch } from './PodcastSearch';
import { RelevantMusic } from './RelevantMusic';
import { useSeenSongs } from './seenBefore';
import { MUSIC_VIEWS, pill } from './searchTypes';
import type { MusicView } from './searchTypes';
import { SourceMenu } from './SourceMenu';
import { playResults, useOnlineSearch } from './useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';

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

/**
 * Music results (M46.9): Relevant (M46.25: a bit of everything), songs, artists, albums or
 * podcasts. Songs are your library's, then the best matches from every source (each naming its
 * source), or each source's own list. `compact` keeps it short for the All view (top results,
 * else the first provider's). The pill lives in the address (`view`).
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
  const results = search.data?.providers ?? [];
  const failed = results.filter((result) => result.error);
  const pending = search.data?.pending?.length ?? 0;
  const library = search.data?.library ?? [];
  const top = search.data?.top ?? [];
  const names = results.map((result) => result.provider.name);
  const nothingFound =
    library.length === 0 &&
    results.length > 0 &&
    results.every((result) => !result.error && result.tracks.length === 0);

  if (compact && search.error) {
    return (
      <SourceIssue>{t('common.coreUnavailable', { message: search.error.message })}</SourceIssue>
    );
  }
  if (compact) {
    const first = results.find((result) => result.tracks.length > 0);
    if (top.length > 0) return <TopResults tracks={top.slice(0, 5)} />;
    if (first) return <ProviderResults result={first} ambiguous={false} limit={5} />;
    return search.isSuccess ? (
      <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
    ) : (
      <p className="text-muted text-sm">{t('search.searching')}</p>
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
      {blank && view !== 'podcasts' && (
        <p className="text-muted text-sm">{t('search.hintFor.music')}</p>
      )}
      {!blank && view === 'relevant' && <RelevantMusic query={query} onView={setView} />}
      {!blank && view === 'artists' && <ArtistFinds query={query} />}
      {!blank && view === 'albums' && <AlbumFinds query={query} />}
      {!blank && view === 'playlists' && <PlaylistFinds query={query} />}
      {!blank && view === 'songs' && search.error && (
        <SourceIssue>{t('common.coreUnavailable', { message: search.error.message })}</SourceIssue>
      )}
      {!blank && view === 'songs' && !search.error && (
        <>
          {search.isSuccess && results.length === 0 && library.length === 0 && (
            <p className="text-muted text-sm">{t('search.noSources')}</p>
          )}
          {library.length > 0 && <LibraryResults tracks={library} />}
          {search.isPending && (
            <p role="status" className="text-muted flex items-center gap-2 text-sm">
              <LoaderCircle aria-hidden size={14} className="animate-spin" />
              {seen.length > 0 ? t('search.seenWhileSearching') : t('search.searching')}
            </p>
          )}
          {search.isPending && seen.length > 0 && (
            <BestSongs query={query} top={seen} results={[]} />
          )}
          {/* Slow sources answer on the next ask (M46.48), a moment later. */}
          {pending > 0 && (
            <p role="status" className="text-muted flex items-center gap-2 text-xs">
              <LoaderCircle aria-hidden size={12} className="animate-spin" />
              {t('search.sourcesAnswered', {
                done: results.length,
                total: results.length + pending,
              })}
            </p>
          )}
          {nothingFound ? (
            <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
          ) : grouped ? (
            <>
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
              <BestSongs
                query={query}
                top={top}
                results={results.filter((result) => !result.error)}
              />
              {/* A quiet mark: what each one met shows when grouped by source. */}
              {failed.length > 0 && (
                <p className="text-muted text-xs wrap-anywhere">
                  {t('search.someFailed', {
                    sources: failed.map((result) => result.provider.name).join(', '),
                  })}
                </p>
              )}
            </>
          )}
        </>
      )}
    </div>
  );
};
