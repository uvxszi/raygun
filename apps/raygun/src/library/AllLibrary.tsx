import type { MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ChevronRight, Heart, Library, Search } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { useFavorites, usePlaylists } from '../collections/useCollections';
import { useLayout } from '../layout/useLayout';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { PodcastArt } from '../podcasts/PodcastArt';
import { Carousel } from '../shell/Carousel';
import { EmptyState } from '../shell/EmptyState';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { LibraryTop } from './LibraryTop';
import type { LibraryMediaTab } from './LibraryTop';

/** Items each row of the All tab shows (M46.37). */
const ROW_ITEMS = 16;

const TILE = 'flex w-28 shrink-0 flex-col gap-1.5 rounded-xl text-sm';

/** One kind's row, with See all leading to its tab. */
const Row: FC<{ title: string; media: LibraryMediaTab; children: ReactNode }> = ({
  title,
  media,
  children,
}) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">{title}</h2>
        <Link
          to="/library"
          search={{ media }}
          className="text-muted hover:text-foreground flex items-center gap-0.5 text-sm font-semibold"
        >
          {t('search.seeAll')}
          <ChevronRight aria-hidden size={16} />
        </Link>
      </div>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">{children}</Carousel>
    </section>
  );
};

/** Titles read or watched lately first, then the latest added. */
const recent = (entries: MangaLibraryEntry[] | undefined) =>
  [...(entries ?? [])]
    .sort(
      (a, b) => (b.lastReadAt ?? 0) - (a.lastReadAt ?? 0) || (b.addedAt ?? 0) - (a.addedAt ?? 0),
    )
    .slice(0, ROW_ITEMS);

const TitleRow: FC<{ medium: 'manga' | 'novel' | 'video'; title: string }> = ({
  medium,
  title,
}) => {
  const library = useMangaLibrary(medium);
  const entries = recent(library.data);
  if (entries.length === 0) return null;
  return (
    <Row title={title} media={medium}>
      {entries.map((entry) => {
        const link =
          medium === 'video'
            ? videoTitleLink(entry.sourceId, entry.remoteId)
            : { to: ROUTES[medium].title, search: { source: entry.sourceId, id: entry.remoteId } };
        return (
          <li key={entry.titleId} className="w-28 shrink-0">
            <Link
              to={link.to}
              search={link.search as never}
              aria-label={entry.title}
              className="flex flex-col gap-1.5"
            >
              <MangaCover
                url={entry.coverUrl}
                video={medium === 'video'}
                className="w-full rounded-xl"
              />
              <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
            </Link>
          </li>
        );
      })}
    </Row>
  );
};

/** Audio's row: Favorites, playlists and followed podcasts. */
const AudioRow: FC = () => {
  const { t } = useTranslation();
  const favorites = useFavorites();
  const playlists = usePlaylists();
  const podcasts = useSubscriptions();
  return (
    <Row title={t('library.tabs.music')} media="music">
      <li className="w-28 shrink-0">
        <Link to="/library" search={{ view: 'favorites' }} className={TILE}>
          <span className="bg-primary/15 text-primary grid aspect-square place-items-center rounded-xl">
            <Heart aria-hidden size={30} fill="currentColor" />
          </span>
          <span className="truncate font-medium">{t('collections.favorites')}</span>
          <span className="text-muted -mt-1 text-xs">
            {t('library.trackCount', { count: favorites.data?.length ?? 0 })}
          </span>
        </Link>
      </li>
      {playlists.data?.slice(0, ROW_ITEMS).map((playlist) => (
        <li key={playlist.id} className="w-28 shrink-0">
          <Link to="/library/playlist" search={{ id: playlist.id }} className={TILE}>
            <PlaylistCover
              artworkUrls={playlist.artworkUrls}
              className="aspect-square w-full rounded-xl"
            />
            <span className="truncate font-medium">{playlist.name}</span>
          </Link>
        </li>
      ))}
      {podcasts.data?.slice(0, ROW_ITEMS).map((show) => (
        <li key={show.id} className="w-28 shrink-0">
          <Link to="/podcast" search={{ id: show.id }} className={TILE}>
            <PodcastArt url={show.artworkUrl} className="w-full rounded-xl" />
            <span className="truncate font-medium">{show.title}</span>
          </Link>
        </li>
      ))}
    </Row>
  );
};

/**
 * Library → All (M46.37): a row of each kind the user shows, in their order — Audio (favorites,
 * playlists, podcasts), Comics, Books and Videos, read or watched lately first — each leading to
 * its own tab.
 */
export const AllLibrary: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const favorites = useFavorites();
  const playlists = usePlaylists();
  const podcasts = useSubscriptions();
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const videos = useMangaLibrary('video');
  const lists = [favorites, playlists, podcasts, manga, novels, videos];
  const empty =
    lists.every((list) => list.data !== undefined) &&
    lists.every((list) => (list.data?.length ?? 0) === 0);
  const rows: Record<string, ReactNode> = {
    music: <AudioRow key="music" />,
    manga: <TitleRow key="manga" medium="manga" title={t('library.tabs.manga')} />,
    novel: <TitleRow key="novel" medium="novel" title={t('library.tabs.novel')} />,
    video: <TitleRow key="video" medium="video" title={t('library.tabs.video')} />,
  };
  return (
    <>
      <LibraryTop media="all" />
      <div data-testid="library-all" className="flex flex-col gap-6">
        {empty && (
          <EmptyState
            icon={Library}
            title={t('library.emptyTitle')}
            body={t('library.allEmptyHint')}
            action={{
              label: t('library.findInBrowse'),
              icon: Search,
              to: '/browse',
              search: { tab: 'search' },
            }}
          />
        )}
        {!empty &&
          layout.libraryMedia
            .filter((item) => item.visible && item.id !== 'all')
            .map((item) => rows[item.id] ?? null)}
      </div>
    </>
  );
};
