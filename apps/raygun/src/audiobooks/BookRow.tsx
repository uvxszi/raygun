import type { AudiobookResult } from '@raygun/bindings';
import { LoaderCircle } from 'lucide-react';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { formatLength, runtimeMs } from '../podcasts/formatLength';
import { PodcastArt } from '../podcasts/PodcastArt';
import { Carousel } from '../shell/Carousel';
import { SourceIssue } from '../shell/SourceIssue';
import { useBookDirectory } from './useAudiobooks';
import type { BookList } from './useAudiobooks';

/** A book from a directory, as a card (M46.22; also on Home since M46.47). */
export const BookCard: FC<{
  book: AudiobookResult;
  onOpen: (book: AudiobookResult) => void;
  /** In a grid, the card takes its cell's width. */
  grid?: boolean;
  /** Its right-click and long-press menu. */
  onMenu?: (event: MouseEvent) => void;
}> = ({ book, onOpen, grid = false, onMenu }) => {
  const { t } = useTranslation();
  const length = runtimeMs(book.runtime);
  return (
    <li className={grid ? 'min-w-0' : 'w-32 shrink-0 sm:w-40'}>
      <button
        type="button"
        onClick={() => onOpen(book)}
        onContextMenu={onMenu}
        data-testid="book-card"
        title={[book.title, book.author, book.edition].filter(Boolean).join(' · ')}
        className="hover:bg-surface flex w-full flex-col gap-1.5 rounded-xl p-2 text-left"
      >
        <PodcastArt url={book.artworkUrl} className="w-full rounded-lg shadow-lg" />
        <span className="line-clamp-2 text-sm font-semibold">{book.title}</span>
        {book.author && <span className="text-muted -mt-1 truncate text-xs">{book.author}</span>}
        <span className="text-muted truncate text-xs">
          {[book.edition, length ? formatLength(length, t) : book.runtime, book.source]
            .filter(Boolean)
            .join(' · ')}
        </span>
      </button>
    </li>
  );
};

/** A row of a directory's books (popular or newest), more as it scrolls to the end. */
export const BookRow: FC<{
  list: BookList;
  title: string;
  onOpen: (book: AudiobookResult) => void;
}> = ({ list, title, onOpen }) => {
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
                aria-label={t('audiobooks.more')}
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
