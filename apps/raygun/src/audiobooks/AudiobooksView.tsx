import { Link } from '@tanstack/react-router';
import { BookAudio } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PodcastArt } from '../podcasts/PodcastArt';
import { EmptyState } from '../shell/EmptyState';
import { useAudiobooks } from './useAudiobooks';

/** Library → Audio → Audiobooks (M46.22): the books you've added, with how far you are. */
export const AudiobooksView: FC = () => {
  const { t } = useTranslation();
  const books = useAudiobooks();
  if (!books.data) return null;
  if (books.data.length === 0) {
    return (
      <EmptyState
        icon={BookAudio}
        title={t('audiobooks.emptyTitle')}
        body={t('audiobooks.emptyBody')}
        action={{ label: t('audiobooks.browse'), to: '/audiobooks' }}
      />
    );
  }
  return (
    <ul
      data-testid="audiobooks-view"
      className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-7"
    >
      {books.data.map((book) => {
        const share = book.chapters > 0 ? book.finished / book.chapters : 0;
        return (
          <li key={book.id}>
            <Link
              to="/audiobook"
              search={{ id: book.id }}
              data-testid="audiobook-card"
              className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
            >
              <PodcastArt url={book.artworkUrl} className="w-full rounded-lg shadow-lg" />
              <span className="line-clamp-2 text-sm font-semibold">{book.title}</span>
              {book.author && (
                <span className="text-muted -mt-1 truncate text-xs">{book.author}</span>
              )}
              <span className="text-muted text-xs">
                {book.finished >= book.chapters && book.chapters > 0
                  ? t('audiobooks.finished')
                  : t('audiobooks.chapterOf', {
                      chapter: Math.min(book.finished + 1, book.chapters),
                      count: book.chapters,
                    })}
              </span>
              <span className="bg-foreground/10 h-1 overflow-hidden rounded-full">
                <span
                  className="bg-primary block h-full"
                  style={{ width: `${Math.round(share * 100)}%` }}
                />
              </span>
            </Link>
          </li>
        );
      })}
    </ul>
  );
};
