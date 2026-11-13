import type { MangaLibraryEntry, Website } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { CheckCheck, ChevronRight, Library, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useAudiobooks } from '../audiobooks/useAudiobooks';
import { PlaylistCover } from '../collections/PlaylistCover';
import { useFavorites, usePlaylists } from '../collections/useCollections';
import { useLayout } from '../layout/useLayout';
import { useLocalFolders } from '../localFiles/useLocalFolders';
import { bookWords } from '../manga/bookWords';
import { SelectionTopBar } from '../manga/LibrarySelection';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { PodcastArt } from '../podcasts/PodcastArt';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { PickTarget } from '../select/PickTarget';
import { usePickList } from '../select/usePickList';
import type { PickState } from '../select/usePickPress';
import { CardMenu } from '../shell/CardMenu';
import { Carousel } from '../shell/Carousel';
import { openMenuAtPoint } from '../shell/contextMenuStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { EmptyState } from '../shell/EmptyState';
import { LetterAvatar } from '../shell/LetterAvatar';
import { SourceIssue } from '../shell/SourceIssue';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { RemoveWebsiteSheet, WebsiteSheet } from '../websites/WebsiteSheets';
import { useOpenWebsite, useWebsites } from '../websites/useWebsites';
import { allItemKey, allItemName } from './allItems';
import type { AllItem } from './allItems';
import { AllSelectionActions } from './AllSelectionActions';
import { LibraryTop } from './LibraryTop';
import type { LibraryMediaTab } from './LibraryTop';
import { DownloadedTile, FavoritesTile, LocalFilesTile } from './LocalFilesTile';
import { useAllItemMenu } from './useAllItemMenu';
import { useLibraryStatus } from './useLibrary';

/** Items each row of the All tab shows (M46.37). */
const ROW_ITEMS = 16;

const TILE = 'flex w-28 shrink-0 flex-col gap-1.5 rounded-xl text-sm';

/** The rows All can show, by their layout ids. */
type RowId = 'music' | 'manga' | 'novel' | 'video' | 'websites';

const ROW_IDS: RowId[] = ['music', 'manga', 'novel', 'video', 'websites'];

const isRowId = (id: string): id is RowId => (ROW_IDS as string[]).includes(id);

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

const titleItems = (medium: Medium, entries: MangaLibraryEntry[] | undefined): AllItem[] =>
  recent(entries).map((entry) => ({ kind: 'title', medium, entry }));

/** Where an item other than a site opens. */
const linkOf = (item: Exclude<AllItem, { kind: 'website' }>) => {
  switch (item.kind) {
    case 'playlist':
      return { to: '/library/playlist', search: { id: item.playlist.id } };
    case 'podcast':
      return { to: '/podcast', search: { id: item.show.id } };
    case 'audiobook':
      return { to: '/audiobook', search: { id: item.book.id } };
    case 'title':
      return item.medium === 'video'
        ? videoTitleLink(item.entry.sourceId, item.entry.remoteId)
        : {
            to: ROUTES[item.medium].title,
            search: { source: item.entry.sourceId, id: item.entry.remoteId },
          };
  }
};

const Cover: FC<{ item: AllItem }> = ({ item }) => {
  switch (item.kind) {
    case 'playlist':
      return (
        <PlaylistCover
          artworkUrls={item.playlist.artworkUrls}
          artworkSongs={item.playlist.artworkSongs}
          coverUrl={item.playlist.coverUrl}
          className="aspect-square w-full rounded-xl"
        />
      );
    case 'podcast':
      return <PodcastArt url={item.show.artworkUrl} className="w-full rounded-xl" />;
    case 'audiobook':
      return <PodcastArt url={item.book.artworkUrl} className="w-full rounded-xl" />;
    case 'title':
      return (
        <MangaCover
          url={item.entry.coverUrl}
          video={item.medium === 'video'}
          book={bookWords(item.medium, item.entry.title, item.entry.authors)}
          className="w-full rounded-xl"
        />
      );
    case 'website':
      return (
        <span className="bg-surface grid aspect-square w-full place-items-center rounded-xl">
          <LetterAvatar name={item.site.name} logo={item.site.icon} size="lg" />
        </span>
      );
  }
};

/**
 * An item of All: its cover and name. Its circle, a Ctrl or Shift-click (and, while picking, a
 * tap) picks it; its menu opens from its ⋮, a hold or a right click.
 */
const ItemTile: FC<{
  item: AllItem;
  pick: PickState;
  menu: (select: (() => void) | null) => MenuItem[];
  onOpenSite: (site: Website) => void;
}> = ({ item, pick, menu, onOpenSite }) => {
  const { t } = useTranslation();
  const name = allItemName(item);
  const items = () => menu(pick.selected ? null : pick.toggle);
  const label = (
    <span
      className={item.kind === 'title' ? 'line-clamp-2 font-medium' : 'w-full truncate font-medium'}
    >
      {name}
    </span>
  );
  return (
    <li className="w-28 shrink-0">
      <PickTarget
        pick={pick}
        label={t('search.select.pick', { title: name })}
        testId="all-item"
        onMenu={(at) => openMenuAtPoint(at, items())}
      >
        {item.kind === 'website' ? (
          <button
            type="button"
            data-pick-main
            aria-label={t('websites.open', { name })}
            aria-pressed={pick.selecting ? pick.selected : undefined}
            onClick={() => onOpenSite(item.site)}
            className={`${TILE} w-full items-center text-center`}
          >
            <Cover item={item} />
            {label}
          </button>
        ) : (
          <Link
            to={linkOf(item).to}
            search={linkOf(item).search as never}
            draggable={false}
            aria-label={name}
            aria-pressed={pick.selecting ? pick.selected : undefined}
            className={TILE}
          >
            <Cover item={item} />
            {label}
          </Link>
        )}
        <CardMenu label={t('menu.options', { name })} items={items} />
      </PickTarget>
    </li>
  );
};

/**
 * Library → All (M46.37): a row of each kind the user shows, in their order — Audio (favorites,
 * playlists, podcasts, audiobooks), Comics, Books, Videos and Websites, read or watched lately
 * first — each leading to its own tab. Every item can be picked as in other lists, with a bar of
 * what the picked kinds share (`AllSelectionActions`); a hold or a right click on an item opens
 * its own menu, as its ⋮ does.
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
  const open = useOpenWebsite();
  const [editing, setEditing] = useState<Website | null>(null);
  const [removing, setRemoving] = useState<Website | null>(null);
  const openSite = (site: Website) => open.mutate(site.id);
  const itemMenu = useAllItemMenu({ open: openSite, edit: setEditing, remove: setRemoving });

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

  // What each row shows, and every item in the order shown (for ranges and Select all).
  const shown = useMemo<Record<RowId, AllItem[]>>(
    () => ({
      music: [
        ...(playlists.data ?? [])
          .slice(0, ROW_ITEMS)
          .map((playlist): AllItem => ({ kind: 'playlist', playlist })),
        ...(podcasts.data ?? [])
          .slice(0, ROW_ITEMS)
          .map((show): AllItem => ({ kind: 'podcast', show })),
        ...(audiobooks.data ?? [])
          .slice(0, ROW_ITEMS)
          .map((book): AllItem => ({ kind: 'audiobook', book })),
      ],
      manga: titleItems('manga', manga.data),
      novel: titleItems('novel', novels.data),
      video: titleItems('video', videos.data),
      websites: (websites.data ?? [])
        .slice(0, ROW_ITEMS)
        .map((site): AllItem => ({ kind: 'website', site })),
    }),
    [
      playlists.data,
      podcasts.data,
      audiobooks.data,
      manga.data,
      novels.data,
      videos.data,
      websites.data,
    ],
  );
  const rowOrder = useMemo(
    () =>
      layout.libraryMedia
        .filter((item) => item.visible && item.id !== 'all')
        .map((item) => item.id)
        .filter(isRowId),
    [layout.libraryMedia],
  );
  const ordered = useMemo(
    () => (empty ? [] : rowOrder.flatMap((id) => shown[id])),
    [empty, shown, rowOrder],
  );
  const order = useMemo(() => ordered.map(allItemKey), [ordered]);
  const picks = usePickList(order);
  const picked = useMemo(
    () => ordered.filter((item) => picks.picked.has(allItemKey(item))),
    [ordered, picks.picked],
  );

  const tiles = (items: AllItem[]) =>
    items.map((item) => {
      const key = allItemKey(item);
      return (
        <ItemTile
          key={key}
          item={item}
          pick={picks.state(key)}
          menu={(select) => itemMenu(item, select)}
          onOpenSite={openSite}
        />
      );
    });
  const titleRow = (id: 'manga' | 'novel' | 'video') =>
    shown[id].length > 0 && (
      <Row key={id} title={t(`library.tabs.${id}`)} media={id}>
        {tiles(shown[id])}
      </Row>
    );
  const opened = websites.data?.find((site) => site.id === open.variables);
  const rows: Record<RowId, ReactNode> = {
    // Favorites, Downloaded and Local files always lead Audio's row (M46.29).
    music: (
      <Row key="music" title={t('library.tabs.music')} media="music">
        <FavoritesTile menuOnHold />
        <DownloadedTile menuOnHold />
        <LocalFilesTile menuOnHold />
        {tiles(shown.music)}
      </Row>
    ),
    manga: titleRow('manga'),
    novel: titleRow('novel'),
    video: titleRow('video'),
    websites: shown.websites.length > 0 && (
      <Row key="websites" title={t('library.tabs.websites')} media="websites">
        {tiles(shown.websites)}
      </Row>
    ),
  };

  return (
    <>
      {picks.selecting ? (
        <SelectionTopBar
          count={picks.picked.size}
          onSelectAll={picks.selectAll}
          onInvert={picks.invert}
          onClose={picks.clear}
        />
      ) : (
        <LibraryTop
          media="all"
          menu={
            ordered.length > 0
              ? [{ label: t('library.select.all'), icon: CheckCheck, onSelect: picks.selectAll }]
              : []
          }
        />
      )}
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
        {!empty && rowOrder.map((id) => rows[id])}
        {open.error && (
          <SourceIssue compact title={t('websites.openFailed', { name: opened?.name ?? '' })}>
            {open.error.message}
          </SourceIssue>
        )}
      </div>
      {picks.selecting && <AllSelectionActions items={picked} onDone={picks.clear} />}
      {editing && <WebsiteSheet site={editing} onClose={() => setEditing(null)} />}
      {removing && <RemoveWebsiteSheet site={removing} onClose={() => setRemoving(null)} />}
    </>
  );
};
