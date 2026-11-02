import type { LibraryTrack, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { OnlineTrackRow } from './OnlineTrackRow';
import { SourceMenu } from './SourceMenu';
import { playResults, useOnlineSearch } from './useOnlineSearch';

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
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('search.providerFailed', { message: result.error })}
        </p>
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

/**
 * Music results: your library's tracks, songs several providers agree on, then each provider.
 * `compact` keeps it short for the All view (top results, else the first provider's).
 */
export const MusicResults: FC<{ query: string; compact?: boolean }> = ({ query, compact }) => {
  const { t } = useTranslation();
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
      <p role="alert" className="text-primary text-sm wrap-anywhere">
        {t('common.coreUnavailable', { message: search.error.message })}
      </p>
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
  return (
    <div className="flex flex-col gap-6">
      {search.isSuccess && results.length === 0 && library.length === 0 && (
        <p className="text-muted text-sm">{t('search.noSources')}</p>
      )}
      {library.length > 0 && <LibraryResults tracks={library} />}
      {top.length > 0 && <TopResults tracks={top} />}
      {nothingFound ? (
        <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
      ) : (
        results.map((result) => (
          <ProviderResults
            key={result.provider.id}
            result={result}
            ambiguous={names.filter((name) => name === result.provider.name).length > 1}
          />
        ))
      )}
    </div>
  );
};
