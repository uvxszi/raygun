import type { AudiobookResult } from '@raygun/bindings';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { BookCard, BookRow } from '../audiobooks/BookRow';
import { useBookDirectory } from '../audiobooks/useAudiobooks';
import type { BookList, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import { LoadMore } from '../shell/LoadMore';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { useDragScroll } from '../shell/useDragScroll';
import { useOpenBook } from './useOpenBook';

/** The directories asked; those waiting for a key are counted in one line. */
const BOOK_KINDS = ['audiobook.directory'];

/** What opening a book shows: a line while it opens, a calm note when it couldn't. */
export const OpeningBook: FC<{ open: ReturnType<typeof useOpenAudiobook> }> = ({ open }) => {
  const { t } = useTranslation();
  if (open.isPending) {
    return (
      <p role="status" className="text-muted flex items-center gap-2 text-sm">
        <LoaderCircle aria-hidden size={16} className="animate-spin" />
        {t('audiobooks.opening')}
      </p>
    );
  }
  return open.error ? <SourceIssue>{open.error.message}</SourceIssue> : null;
};

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

/**
 * Every source's books for a search (ranked by title and author, a book two sources share shown
 * once, late sources added as they answer) or a genre, as a grid, more as you go.
 */
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
      {results.isPending && (
        <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />
      )}
      {results.error && (
        <SourceIssue onRetry={() => void results.refetch()}>{results.error.message}</SourceIssue>
      )}
      {results.isSuccess && found.length === 0 && (
        <p className="text-muted text-sm">{t('search.noResults', { query })}</p>
      )}
      {found.length > 0 && (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(8rem,1fr))] gap-2 sm:grid-cols-[repeat(auto-fill,minmax(10rem,1fr))]">
          {found.map((book) => (
            <BookCard key={book.feedUrl} book={book} onOpen={onOpen} grid />
          ))}
        </ul>
      )}
      <LoadMore
        hasNextPage={results.hasNextPage}
        fetching={results.isFetchingNextPage}
        onLoad={() => void results.fetchNextPage()}
      />
      {!results.isPending && <NeedsSetupNote kinds={BOOK_KINDS} />}
    </section>
  );
};

/**
 * Search → Audio → Audiobooks: every audiobook source's books for the query, more as you go;
 * before anything is typed, a genre's books, or their popular and newest books.
 */
export const AudiobookSearch: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const wanted = query.trim();
  const { openBook, open } = useOpenBook();
  const [genre, setGenre] = useState<string | null>(null);
  const picked = GENRES.find(([id]) => id === genre);
  const genreRow = useDragScroll<HTMLDivElement>();

  return (
    <div data-testid="audiobook-search" className="flex flex-col gap-6">
      <OpeningBook open={open} />
      {wanted ? (
        <BookGrid list="search" query={wanted} label={t('audiobooks.results')} onOpen={openBook} />
      ) : (
        <>
          <div
            ref={genreRow}
            role="group"
            aria-label={t('audiobooks.genresLabel')}
            className="no-scrollbar -mx-4 flex gap-2 overflow-x-auto px-4"
          >
            {GENRES.map(([id]) => (
              <button
                key={id}
                type="button"
                aria-pressed={genre === id}
                onClick={() => setGenre(genre === id ? null : id)}
                className={`shrink-0 rounded-full px-3.5 py-1.5 text-sm ${genre === id ? 'bg-primary text-background font-semibold' : 'bg-surface'}`}
              >
                {t(`audiobooks.genres.${id}`)}
              </button>
            ))}
          </div>
          {picked ? (
            <BookGrid
              list="subject"
              query={picked[1]}
              label={t(`audiobooks.genres.${picked[0]}`)}
              onOpen={openBook}
            />
          ) : (
            <>
              <BookRow list="popular" title={t('audiobooks.popular')} onOpen={openBook} />
              <BookRow list="latest" title={t('audiobooks.latest')} onOpen={openBook} />
            </>
          )}
        </>
      )}
    </div>
  );
};
