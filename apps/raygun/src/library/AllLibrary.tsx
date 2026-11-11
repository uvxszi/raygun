import type { MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ChevronRight, Library, Search } from 'lucide-react';
import { useMemo } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useAudiobooks } from '../audiobooks/useAudiobooks';
import { PlaylistCover } from '../collections/PlaylistCover';
import { useFavorites, usePlaylists } from '../collections/useCollections';
import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import { useLayout } from '../layout/useLayout';
import { useLocalFolders } from '../localFiles/useLocalFolders';
import { bookWords } from '../manga/bookWords';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { PodcastArt } from '../podcasts/PodcastArt';
import { Carousel } from '../shell/Carousel';
import { EmptyState } from '../shell/EmptyState';
import { LetterAvatar } from '../shell/LetterAvatar';
import { SourceIssue } from '../shell/SourceIssue';
import { useOpenWebsite, useWebsites } from '../websites/useWebsites';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { DownloadedTile, FavoritesTile, LocalFilesTile } from './LocalFilesTile';
import { LibraryTop } from './LibraryTop';
import type { LibraryMediaTab } from './LibraryTop';
import { useLibraryStatus } from './useLibrary';
import { CardMenu } from '../shell/CardMenu';

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
  const entries = useMemo(() => recent(library.data), [library.data]);
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
                book={bookWords(medium, entry.title, entry.authors)}
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

/** Websites' row: each site opens in the built-in browser. */
const WebsitesRow: FC = () => {
  const { t } = useTranslation();
  const websites = useWebsites();
  const open = useOpenWebsite();
  if (!websites.data?.length) return null;
  const opened = websites.data.find((site) => site.id === open.variables);
  return (
    <>
      <Row title={t('library.tabs.websites')} media="websites">
        {websites.data.slice(0, ROW_ITEMS).map((site) => (
          <li key={site.id} className="w-28 shrink-0">
            <button
              type="button"
              aria-label={t('websites.open', { name: site.name })}
              onClick={() => open.mutate(site.id)}
              className={`${TILE} items-center text-center`}
            >
              <span className="bg-surface grid aspect-square w-full place-items-center rounded-xl">
                <LetterAvatar name={site.name} logo={site.icon} size="lg" />
              </span>
              <span className="w-full truncate font-medium">{site.name}</span>
            </button>
          </li>
        ))}
      </Row>
      {open.error && (
        <SourceIssue compact title={t('websites.openFailed', { name: opened?.name ?? '' })}>
          {open.error.message}
        </SourceIssue>
      )}
    </>
  );
};

/**
 * Audio's row: Favorites, Downloaded and Local files (always first, M46.29), then playlists,
 * followed podcasts and audiobooks.
 */
const AudioRow: FC = () => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const playlistMenu = usePlaylistMenuItems();
  const podcasts = useSubscriptions();
  const audiobooks = useAudiobooks();
  return (
    <Row title={t('library.tabs.music')} media="music">
      <FavoritesTile />
      <DownloadedTile />
      <LocalFilesTile />
      {playlists.data?.slice(0, ROW_ITEMS).map((playlist) => (
        <li key={playlist.id} className="w-28 shrink-0">
          <Link to="/library/playlist" search={{ id: playlist.id }} className={`relative ${TILE}`}>
            <CardMenu
              label={t('menu.options', { name: playlist.name })}
              items={() => playlistMenu(playlist)}
            />
            <PlaylistCover
              artworkUrls={playlist.artworkUrls}
              artworkSongs={playlist.artworkSongs}
              coverUrl={playlist.coverUrl}
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
      {audiobooks.data?.slice(0, ROW_ITEMS).map((book) => (
        <li key={book.id} className="w-28 shrink-0">
          <Link to="/audiobook" search={{ id: book.id }} className={TILE}>
            <PodcastArt url={book.artworkUrl} className="w-full rounded-xl" />
            <span className="truncate font-medium">{book.title}</span>
          </Link>
        </li>
      ))}
    </Row>
  );
};

/**
 * Library → All (M46.37): a row of each kind the user shows, in their order — Audio (favorites,
 * playlists, podcasts), Comics, Books, Videos and Websites, read or watched lately first — each leading to
 * its own tab.
 */
export const AllLibrary: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const favorites = useFavorites();
  const playlists = usePlaylists();
  const podcasts = useSubscriptions();
  const audiobooks = useAudiobooks();
  const downloaded = useDownloadedSongs();
  const status = useLibraryStatus();
  // Followed here too: a scan that ends while this shows fills the rows below.
  const folders = useLocalFolders();
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const videos = useMangaLibrary('video');
  const websites = useWebsites();
  const lists = [
    favorites,
    playlists,
    podcasts,
    audiobooks,
    downloaded,
    manga,
    novels,
    videos,
    websites,
  ];
  // Empty only when every kind answered with nothing, the device's own songs included; a list
  // that failed to load counts as nothing rather than leaving the page blank.
  const empty =
    lists.every((list) => list.data !== undefined || list.isError) &&
    lists.every((list) => (list.data?.length ?? 0) === 0) &&
    (status.data !== undefined || status.isError) &&
    (status.data?.trackCount ?? 0) === 0 &&
    (folders.data !== undefined || folders.isError) &&
    (folders.data?.length ?? 0) === 0;
  const rows: Record<string, ReactNode> = {
    music: <AudioRow key="music" />,
    manga: <TitleRow key="manga" medium="manga" title={t('library.tabs.manga')} />,
    novel: <TitleRow key="novel" medium="novel" title={t('library.tabs.novel')} />,
    video: <TitleRow key="video" medium="video" title={t('library.tabs.video')} />,
    websites: <WebsitesRow key="websites" />,
  };
  return (
    <>
      <LibraryTop media="all" />
      <div data-testid="library-all" className="flex flex-col gap-6">
        {empty && (
          <EmptyState
            icon={Library}
            title={t('library.emptyTitle')}
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
