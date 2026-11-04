import type { AudiobookResult } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Search, X } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { BookCard, BookRow } from '../audiobooks/BookRow';
import { useBookDirectory, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import type { BookList } from '../audiobooks/useAudiobooks';
import { LoadMore } from '../shell/LoadMore';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';

/** Genres every audiobook source matches to its own, by their English names (M46.48). */
const GENRES = [
  ['mystery', 'Mystery'],
  ['children', 'Children'],
  ['fantasy', 'Fantasy'],
  ['scienceFiction', 'Science fiction'],
  ['adventure', 'Adventure'],
  ['romance', 'Romance'],
  ['horror', 'Horror'],
  ['humor', 'Humor'],
  ['historicalFiction', 'Historical fiction'],
  ['fairyTales', 'Fairy tales'],
  ['shortStories', 'Short stories'],
  ['poetry', 'Poetry'],
  ['history', 'History'],
  ['philosophy', 'Philosophy'],
] as const;

/** A directory list as a grid of covers, more as it scrolls to the end. */
const BookGrid: FC<{
  list: BookList;
  query: string;
  label: string;
  onOpen: (book: AudiobookResult) => void;
}> = ({ list, query, label, onOpen }) => {
  const { t } = useTranslation();
  const results = useBookDirectory(list, query);
  const found = results.data?.pages.flatMap((page) => page.books) ?? [];
  return (
    <section aria-label={label} className="flex flex-col gap-2">
      <h2 className="text-lg font-semibold">{label}</h2>
      {results.isPending && (
        <LoaderCircle aria-label={t('audiobooks.loading')} className="text-muted animate-spin" />
      )}
      {results.error && (
        <SourceIssue onRetry={() => void results.refetch()}>{results.error.message}</SourceIssue>
      )}
      {results.isSuccess && found.length === 0 && (
        <p className="text-muted text-sm">{t('audiobooks.noResults')}</p>
      )}
      <ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
        {found.map((book) => (
          <BookCard key={book.feedUrl} book={book} onOpen={onOpen} grid />
        ))}
      </ul>
      <LoadMore
        hasNextPage={results.hasNextPage}
        fetching={results.isFetchingNextPage}
        onLoad={() => void results.fetchNextPage()}
      />
    </section>
  );
};

/**
 * Audiobooks to find (M46.22): search every audiobook source at once (LibriVox, Loyal Books,
 * Lit2Go…), browse a genre, or pick from the popular and newest books. A book opens on its page,
 * ready to play or add.
 */
export const AudiobooksBrowsePage: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const open = useOpenAudiobook();
  const [draft, setDraft] = useState('');
  const [query, setQuery] = useState('');
  const [genre, setGenre] = useState<string | null>(null);
  const openBook = (book: AudiobookResult) =>
    open.mutate(book.feedUrl, {
      onSuccess: (id) => void navigate({ to: '/audiobook', search: { id } }),
    });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setQuery(draft.trim());
    if (draft.trim()) setGenre(null);
  };
  const genreName = GENRES.find(([id]) => id === genre);

  return (
    <section data-testid="audiobooks-browse" className="flex flex-col gap-6">
      <TopBar back fallback="/browse" title={t('audiobooks.title')} />
      <form onSubmit={submit} className="bg-surface flex items-center gap-2 rounded-full px-4 py-2">
        <Search aria-hidden size={16} className="text-muted" />
        <input
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          aria-label={t('audiobooks.search')}
          placeholder={t('audiobooks.searchPlaceholder')}
          className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
        />
        {draft && (
          <button
            type="button"
            aria-label={t('nav.clearSearch')}
            onClick={() => {
              setDraft('');
              setQuery('');
            }}
            className="text-muted hover:text-foreground"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </form>
      <div
        role="group"
        aria-label={t('audiobooks.genresLabel')}
        className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4"
      >
        {GENRES.map(([id]) => (
          <button
            key={id}
            type="button"
            aria-pressed={genre === id}
            onClick={() => {
              setGenre(genre === id ? null : id);
              setQuery('');
              setDraft('');
            }}
            className={`shrink-0 rounded-full px-3.5 py-1.5 text-sm ${genre === id ? 'bg-primary text-background font-semibold' : 'bg-surface'}`}
          >
            {t(`audiobooks.genres.${id}`)}
          </button>
        ))}
      </div>
      {open.isPending && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={16} className="animate-spin" />
          {t('audiobooks.opening')}
        </p>
      )}
      {open.error && <SourceIssue>{open.error.message}</SourceIssue>}
      {query ? (
        <BookGrid list="search" query={query} label={t('audiobooks.results')} onOpen={openBook} />
      ) : genreName ? (
        <BookGrid
          list="subject"
          query={genreName[1]}
          label={t(`audiobooks.genres.${genreName[0]}`)}
          onOpen={openBook}
        />
      ) : (
        <>
          <BookRow list="popular" title={t('audiobooks.popular')} onOpen={openBook} />
          <BookRow list="latest" title={t('audiobooks.latest')} onOpen={openBook} />
        </>
      )}
    </section>
  );
};
