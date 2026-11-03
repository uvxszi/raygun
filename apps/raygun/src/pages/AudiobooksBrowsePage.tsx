import type { AudiobookResult } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC, FormEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { BookCard, BookRow } from '../audiobooks/BookRow';
import { useBookDirectory, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';

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
