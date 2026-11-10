import { Link } from '@tanstack/react-router';
import { BookAudio } from 'lucide-react';
import { useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SelectionTopBar } from '../manga/LibrarySelection';
import { useBookMenuItems } from '../music/cardMenus';
import { formatLength } from '../podcasts/formatLength';
import { PodcastArt } from '../podcasts/PodcastArt';
import { PodcastPickActions } from '../podcasts/PodcastPickActions';
import { searchLink } from '../search/searchTypes';
import { PickTarget } from '../select/PickTarget';
import { usePickList } from '../select/usePickList';
import { withSelect } from '../select/withSelect';
import { EmptyState } from '../shell/EmptyState';
import { useAudiobooks } from './useAudiobooks';
import { CardMenu } from '../shell/CardMenu';
import { CARD_MENU_BESIDE } from '../manga/CardPlayButton';

/**
 * Library → Audio → Audiobooks (M46.22): the books you've added, with how far you are. Books are
 * picked (a finger held on one, Ctrl/Shift-click, the circle on its cover) for the selection's
 * bar: queue, mark finished, download, delete downloads, take out of the library, pin.
 */
export const AudiobooksView: FC = () => {
  const { t } = useTranslation();
  const books = useAudiobooks();
  const menu = useBookMenuItems();
  const order = useMemo(() => (books.data ?? []).map((book) => book.id), [books.data]);
  const picks = usePickList(order);
  if (!books.data) return null;
  if (books.data.length === 0) {
    return (
      <EmptyState
        icon={BookAudio}
        title={t('audiobooks.emptyTitle')}
        body={t('audiobooks.emptyBody')}
        action={{ label: t('audiobooks.browse'), ...searchLink('music', 'audiobooks') }}
      />
    );
  }
  return (
    <div className="flex flex-1 flex-col gap-2">
      {picks.selecting && (
        <SelectionTopBar
          count={picks.picked.size}
          onSelectAll={picks.selectAll}
          onInvert={picks.invert}
          onClose={picks.clear}
        />
      )}
      <ul
        data-testid="audiobooks-view"
        className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5 2xl:grid-cols-7"
      >
        {books.data.map((book) => {
          // By listening time when the feed gives lengths, else by chapters.
          const totalMs = book.totalMs ?? 0;
          const listenedMs = book.listenedMs ?? 0;
          const timed = totalMs > 0;
          const share = timed
            ? listenedMs / totalMs
            : book.chapters > 0
              ? book.finished / book.chapters
              : 0;
          const done = book.finished >= book.chapters && book.chapters > 0;
          const started = book.finished > 0 || listenedMs > 0;
          const pick = picks.state(book.id);
          return (
            <li key={book.id}>
              <PickTarget
                pick={pick}
                label={t('search.select.pick', { title: book.title })}
                // On the cover, inside the card's padding.
                circleClassName="absolute top-3.5 left-3.5"
              >
                <Link
                  to="/audiobook"
                  search={{ id: book.id }}
                  data-testid="audiobook-card"
                  draggable={false}
                  className="relative hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
                >
                  <CardMenu
                    label={t('menu.options', { name: book.title })}
                    items={() =>
                      withSelect(
                        menu({
                          title: book.title,
                          author: book.author,
                          artworkUrl: book.artworkUrl,
                          id: book.id,
                          kept: true,
                        }),
                        t,
                        pick.selected ? null : pick.toggle,
                      )
                    }
                    className={CARD_MENU_BESIDE}
                  />
                  <PodcastArt url={book.artworkUrl} className="w-full rounded-lg shadow-lg" />
                  <span className="line-clamp-2 text-sm font-semibold">{book.title}</span>
                  {(book.author ?? book.edition) && (
                    <span className="text-muted -mt-1 truncate text-xs">
                      {[book.author, book.edition].filter(Boolean).join(' · ')}
                    </span>
                  )}
                  <span className="text-muted text-xs">
                    {done
                      ? t('audiobooks.finished')
                      : [
                          t('audiobooks.chapterOf', {
                            chapter: Math.min(book.finished + 1, book.chapters),
                            count: book.chapters,
                          }),
                          started && timed
                            ? t('podcasts.left', {
                                time: formatLength(totalMs - listenedMs, t),
                              })
                            : null,
                        ]
                          .filter(Boolean)
                          .join(' · ')}
                  </span>
                  <span className="bg-foreground/10 h-1 overflow-hidden rounded-full">
                    <span
                      className="bg-primary block h-full origin-left"
                      style={{ transform: `scaleX(${Math.min(1, Math.max(0, share))})` }}
                    />
                  </span>
                </Link>
              </PickTarget>
            </li>
          );
        })}
      </ul>
      {picks.selecting && (
        <PodcastPickActions
          picks={{ shows: order.filter((id) => picks.has(id)), episodes: [] }}
          books
          onDone={picks.clear}
        />
      )}
    </div>
  );
};
