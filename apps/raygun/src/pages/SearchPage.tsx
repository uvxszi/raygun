import type { ProviderSearchResult } from '@raygun/bindings';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults, useOnlineSearch } from '../search/useOnlineSearch';
import { PageHeader } from './PageHeader';

/** Wait for a pause in typing before searching, so each keystroke doesn't hit the network. */
export const SEARCH_DEBOUNCE_MS = 400;

type ProviderResultsProps = { result: ProviderSearchResult; ambiguous: boolean };

const ProviderResults: FC<ProviderResultsProps> = ({ result, ambiguous }) => {
  const { t } = useTranslation();
  return (
    <section data-testid="provider-results" className="flex flex-col gap-1">
      <h2 className="text-muted text-sm font-medium">
        {result.provider.name}
        {/* Two sources with one name (a Nuclear plugin beside Raygun's own) need telling apart. */}
        {ambiguous && <span className="font-normal"> · {result.provider.extensionId}</span>}
      </h2>
      {result.error ? (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('search.providerFailed', { message: result.error })}
        </p>
      ) : result.tracks.length === 0 ? (
        <p className="text-muted text-sm">{t('search.providerEmpty')}</p>
      ) : (
        <ul className="flex flex-col">
          {result.tracks.map((track, index) => (
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

export const SearchPage: FC = () => {
  const { t } = useTranslation();
  const { q = '' } = useSearch({ from: '/search' });
  const navigate = useNavigate({ from: '/search' });
  const [input, setInput] = useState(q);
  const search = useOnlineSearch(q);

  // The query lives in the URL, so Back to this page shows the same results.
  useEffect(() => {
    if (input.trim() === q) {
      return;
    }
    const timer = setTimeout(() => {
      const next = input.trim();
      void navigate({ search: next ? { q: next } : {}, replace: true });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [input, q, navigate]);

  const results = search.data ?? [];
  const names = results.map((result) => result.provider.name);
  const nothingFound =
    results.length > 0 && results.every((result) => !result.error && result.tracks.length === 0);

  return (
    <section data-testid="search-page" className="flex flex-col gap-6">
      <PageHeader title={t('search.title')} />
      <label className="bg-surface flex items-center gap-3 rounded-xl px-4 py-3">
        <Search aria-hidden size={20} className="text-muted" />
        <input
          type="search"
          enterKeyHint="search"
          value={input}
          onChange={(event) => setInput(event.target.value)}
          placeholder={t('search.placeholder')}
          aria-label={t('search.title')}
          className="placeholder:text-muted w-full bg-transparent outline-none"
        />
        {search.isFetching && (
          <LoaderCircle
            aria-label={t('search.searching')}
            role="status"
            size={20}
            className="text-muted shrink-0 animate-spin"
          />
        )}
      </label>
      {!q && <p className="text-muted text-sm">{t('search.hint')}</p>}
      {search.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('common.coreUnavailable', { message: search.error.message })}
        </p>
      )}
      {search.isSuccess && results.length === 0 && (
        <p className="text-muted text-sm">{t('search.noSources')}</p>
      )}
      {nothingFound ? (
        <p className="text-muted text-sm">{t('search.noResults', { query: q })}</p>
      ) : (
        results.map((result) => (
          <ProviderResults
            key={result.provider.id}
            result={result}
            ambiguous={names.filter((name) => name === result.provider.name).length > 1}
          />
        ))
      )}
    </section>
  );
};
