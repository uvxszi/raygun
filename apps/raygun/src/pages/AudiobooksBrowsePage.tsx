import type { AudiobookResult } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useBookDirectory, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import type { BookList } from '../audiobooks/useAudiobooks';
import { PodcastArt } from '../podcasts/PodcastArt';
import { Carousel } from '../shell/Carousel';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';

const BookCard: FC<{ book: AudiobookResult; onOpen: (book: AudiobookResult) => void }> = ({
  book,
  onOpen,
}) => (
  <li className="w-32 shrink-0 sm:w-40">
    <button
      type="button"
      onClick={() => onOpen(book)}
      data-testid="book-card"
      className="hover:bg-surface flex w-full flex-col gap-1.5 rounded-xl p-2 text-left"
    >
      <PodcastArt url={book.artworkUrl} className="w-full rounded-lg shadow-lg" />
      <span className="line-clamp-2 text-sm font-semibold">{book.title}</span>
      {book.author && <span className="text-muted -mt-1 truncate text-xs">{book.author}</span>}
      {book.runtime && <span className="text-muted text-xs">{book.runtime}</span>}
    </button>
  </li>
);

/** A row of a directory's books (popular or newest), more as it scrolls to the end. */
const BookRow: FC<{ list: BookList; title: string; onOpen: (book: AudiobookResult) => void }> = ({
  list,
  title,
  onOpen,
}) => {
  const { t } = useTranslation();
  const books = useBookDirectory(list);
  const all = books.data?.pages.flatMap((page) => page.books) ?? [];
  return (
    <section aria-label={title} className="flex flex-col gap-1">
      <h2 className="text-lg font-semibold">{title}</h2>
      {books.isPending && (
        <LoaderCircle
          aria-label={t('audiobooks.loadingBooks')}
          className="text-muted animate-spin"
        />
      )}
      {books.error ? (
        <SourceIssue compact onRetry={() => void books.refetch()}>
          {books.error.message}
        </SourceIssue>
      ) : (
        <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
          {all.map((book) => (
            <BookCard key={book.feedUrl} book={book} onOpen={onOpen} />
          ))}
          {books.hasNextPage && (
            <li className="grid w-32 shrink-0 place-items-center">
              <button
                type="button"
                onClick={() => void books.fetchNextPage()}
                className="bg-surface rounded-full px-4 py-2 text-sm"
              >
                {books.isFetchingNextPage ? '…' : '+'}
              </button>
            </li>
          )}
        </Carousel>
      )}
    </section>
  );
};

/**
 * Audiobooks to find (M46.22): search the audiobook sources (LibriVox), or pick from the popular
 * and newest books. A book opens on its page, ready to play or add.
 */
export const AudiobooksBrowsePage: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const open = useOpenAudiobook();
  const [draft, setDraft] = useState('');
  const [query, setQuery] = useState('');
  const results = useBookDirectory('search', query);
  const found = results.data?.pages.flatMap((page) => page.books) ?? [];
  const openBook = (book: AudiobookResult) =>
    open.mutate(book.feedUrl, {
      onSuccess: (id) => void navigate({ to: '/audiobook', search: { id } }),
    });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setQuery(draft.trim());
  };

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
      </form>
      {open.isPending && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={16} className="animate-spin" />
          {t('audiobooks.opening')}
        </p>
      )}
      {open.error && <SourceIssue>{open.error.message}</SourceIssue>}
      {query ? (
        <section aria-label={t('audiobooks.results')} className="flex flex-col gap-2">
          <h2 className="text-lg font-semibold">{t('audiobooks.results')}</h2>
          {results.isPending && (
            <LoaderCircle
              aria-label={t('audiobooks.loading')}
              className="text-muted animate-spin"
            />
          )}
          {results.error && <SourceIssue>{results.error.message}</SourceIssue>}
          {results.isSuccess && found.length === 0 && (
            <p className="text-muted text-sm">{t('audiobooks.noResults')}</p>
          )}
          <ul className="grid grid-cols-2 gap-3 sm:grid-cols-4 lg:grid-cols-6">
            {found.map((book) => (
              <BookCard key={book.feedUrl} book={book} onOpen={openBook} />
            ))}
          </ul>
        </section>
      ) : (
        <>
          <BookRow list="popular" title={t('audiobooks.popular')} onOpen={openBook} />
          <BookRow list="latest" title={t('audiobooks.latest')} onOpen={openBook} />
        </>
      )}
    </section>
  );
};
