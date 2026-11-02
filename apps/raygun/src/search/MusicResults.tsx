import type { LibraryTrack, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { AlbumFinds, ArtistFinds, BestSongs } from './MusicFinds';
import { OnlineTrackRow } from './OnlineTrackRow';
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
        <SourceIssue source={result.provider.name}>
          {t('search.providerFailed', { message: result.error })}
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

type MusicView = 'songs' | 'artists' | 'albums';

const GROUPED_KEY = 'raygun.search.grouped.music';

const readGrouped = () => {
  try {
    return localStorage.getItem(GROUPED_KEY) === '1';
  } catch {
    return false;
  }
};

const pill = (on: boolean) =>
  on
    ? 'bg-primary/20 text-primary rounded-full px-4 py-1.5 text-sm font-semibold'
    : 'bg-surface rounded-full px-4 py-1.5 text-sm';

/**
 * Music results (M46.9): songs, artists or albums. Songs are your library's, then the best
 * matches from every source (each naming its source), or each source's own list.
 * `compact` keeps it short for the All view (top results, else the first provider's).
 */
export const MusicResults: FC<{ query: string; compact?: boolean }> = ({ query, compact }) => {
  const { t } = useTranslation();
  const [view, setView] = useState<MusicView>('songs');
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
  const results = search.data?.providers ?? [];
  const library = search.data?.library ?? [];
  const top = search.data?.top ?? [];
  const names = results.map((result) => result.provider.name);
  const nothingFound =
    library.length === 0 &&
    results.length > 0 &&
    results.every((result) => !result.error && result.tracks.length === 0);

  if (search.error) {
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
  const views: MusicView[] = ['songs', 'artists', 'albums'];
  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center gap-2">
        <div role="group" aria-label={t('search.music.kind')} className="flex gap-2">
          {views.map((option) => (
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
      {view === 'artists' && <ArtistFinds query={query} />}
      {view === 'albums' && <AlbumFinds query={query} />}
      {view === 'songs' && (
        <>
          {search.isSuccess && results.length === 0 && library.length === 0 && (
            <p className="text-muted text-sm">{t('search.noSources')}</p>
          )}
          {library.length > 0 && <LibraryResults tracks={library} />}
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
              {results
                .filter((result) => result.error)
                .map((result) => (
                  <p key={result.provider.id} className="text-muted text-xs wrap-anywhere">
                    {result.provider.name}: {t('search.providerFailed', { message: result.error })}
                  </p>
                ))}
            </>
          )}
        </>
      )}
    </div>
  );
};
