import { commands } from '@raygun/bindings';
import type { MangaLibraryEntry } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Heart, LayoutList, Library, Search, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { SavedTrackRow } from '../collections/SavedTrackRow';
import {
  HOME_HISTORY_LIMIT,
  playCollection,
  fromSavedTrack,
  useClearHistory,
  useFavorites,
  useHistory,
  usePlaylists,
} from '../collections/useCollections';
import { usePlaylistMenu } from '../collections/usePlaylistMenu';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useLayout, useSaveLayout } from '../layout/useLayout';
import { ChartRow } from '../music/ChartRow';
import { MixCard } from '../music/MixCard';
import { useTrackMenu } from '../music/useTrackMenu';
import { pickChart, useMusicCharts } from '../music/useDiscovery';
import { useMadeForYou } from '../music/useMadeForYou';
import { useIncognitoItem } from '../privacy/useIncognito';
import { useSettings } from '../settings/useSettings';
import { HomeFixedTiles } from '../library/LocalFilesTile';
import {
  AudiobooksSection,
  ChartGroupSection,
  ChartTilesSection,
  DecadesSection,
  GenresSection,
  PopularTitlesSection,
} from '../home/MoreRows';
import { MascotButton } from '../shell/MascotButton';
import { OverflowMenu } from '../shell/OverflowMenu';
import { useIsDesktop } from '../shell/useIsDesktop';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { UpdateBanner } from '../updates/UpdateBanner';
import { ContinueWatchingRow } from '../watch/ContinueWatchingRow';
import { useContinueWatching } from '../watch/useWatch';
import { Carousel } from '../shell/Carousel';

/** Playlists shown as shortcuts next to Favorites. */
const HOME_PLAYLISTS = 5;

const TILE_CLASS = 'bg-surface flex w-32 shrink-0 flex-col gap-2 rounded-xl p-2 text-sm';

/** Recently played from (M46.37) and favorite songs shown in Your music. */
const RECENT_SHOWN = 12;
const FAVORITES_SHOWN = 12;

/**
 * Your music (M46.37): Favorites, Downloaded and Local files, then what was played from lately
 * (albums, playlists, mixes, artists, charts, podcasts, books), then playlists not among them,
 * and a row of the latest favorite songs.
 */
const YourMusic: FC = () => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const playlistMenu = usePlaylistMenu();
  const recent = useQuery({
    queryKey: ['recentContexts'],
    queryFn: () => unwrapResult(commands.recentContexts()),
  });
  const favorites = useFavorites();
  const menu = useTrackMenu();
  // Kept only while recently played is on (M46.41).
  const keepRecent = useSettings().data?.privacy.recentlyPlayed !== false;
  const contexts = keepRecent ? (recent.data ?? []).slice(0, RECENT_SHOWN) : [];
  const recentPlaylists = new Set(
    contexts.filter((context) => context.kind === 'playlist').map((context) => context.search.id),
  );
  const latest = [...(favorites.data ?? [])]
    .sort((a, b) => (b.addedAt ?? 0) - (a.addedAt ?? 0))
    .slice(0, FAVORITES_SHOWN);
  return (
    <section data-testid="your-music" className="flex flex-col gap-2">
      <h2 className="text-lg font-semibold">{t('home.yourMusic')}</h2>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        <li>
          <Link to="/library" search={{ view: 'favorites' }} className={TILE_CLASS}>
            <span className="bg-primary/15 text-primary grid aspect-square place-items-center rounded-lg">
              <Heart aria-hidden size={32} fill="currentColor" />
            </span>
            <span className="truncate font-medium">{t('collections.favorites')}</span>
          </Link>
        </li>
        <HomeFixedTiles className={TILE_CLASS} />
        {contexts.map((context) => (
          <li key={`${context.to}?${JSON.stringify(context.search)}`}>
            <Link
              to={context.to}
              search={context.search as never}
              aria-label={`${context.name} · ${t(`home.played.${context.kind}`, { defaultValue: '' })}`}
              className={TILE_CLASS}
            >
              <Artwork
                url={context.artworkUrl}
                className={`w-full ${context.kind === 'artist' ? 'rounded-full' : 'rounded-lg'}`}
                iconSize={32}
              />
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{context.name}</span>
                <span className="text-muted truncate text-xs">
                  {[t(`home.played.${context.kind}`, { defaultValue: '' }), context.subtitle]
                    .filter(Boolean)
                    .join(' · ')}
                </span>
              </span>
            </Link>
          </li>
        ))}
        {playlists.data
          ?.filter((playlist) => !recentPlaylists.has(playlist.id))
          .slice(0, HOME_PLAYLISTS)
          .map((playlist) => (
            <li key={playlist.id}>
              <Link
                to="/library/playlist"
                search={{ id: playlist.id }}
                onContextMenu={playlistMenu(playlist)}
                className={TILE_CLASS}
              >
                <PlaylistCover
                  artworkUrls={playlist.artworkUrls}
                  coverUrl={playlist.coverUrl}
                  className="aspect-square w-full rounded-lg"
                />
                <span className="truncate font-medium">{playlist.name}</span>
              </Link>
            </li>
          ))}
      </Carousel>
      {latest.length > 0 && (
        <>
          <h3 className="text-muted mt-2 text-sm font-semibold">{t('home.latestFavorites')}</h3>
          <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
            {latest.map((track) => (
              <li key={track.trackKey} className="w-28 shrink-0">
                <button
                  type="button"
                  // The same menu as anywhere a song is listed (M46.40).
                  onContextMenu={menu(fromSavedTrack(track), { favorite: true })}
                  onClick={() =>
                    void playCollection({
                      collection: { kind: 'favorites' },
                      startIndex: (favorites.data ?? []).indexOf(track),
                      shuffle: false,
                    })
                  }
                  className="hover:bg-surface flex w-full flex-col gap-1.5 rounded-xl p-1.5 text-left"
                >
                  <Artwork url={track.artworkUrl} className="w-full rounded-lg" iconSize={28} />
                  <span className="truncate text-sm font-medium">{track.title}</span>
                  <span className="text-muted truncate text-xs">{track.artist}</span>
                </button>
              </li>
            ))}
          </Carousel>
        </>
      )}
    </section>
  );
};

const RecentlyPlayed: FC = () => {
  const { t } = useTranslation();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const clear = useClearHistory();
  // Listening history off (M46.41): nothing of it shows.
  const keep = useSettings().data?.privacy.history !== false;
  if (!keep || !history.data?.length) {
    return null;
  }
  return (
    <section data-testid="recently-played" className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">{t('home.recentlyPlayed')}</h2>
        {history.data.length > 0 && (
          <button
            type="button"
            disabled={clear.isPending}
            onClick={() => clear.mutate()}
            className="text-muted text-sm"
          >
            {t('home.clearHistory')}
          </button>
        )}
      </div>
      {history.data.length === 0 ? (
        <p className="text-muted text-sm">{t('home.noHistory')}</p>
      ) : (
        <ul className="flex flex-col">
          {history.data.map((entry, index) => (
            <li key={entry.track.trackKey}>
              <SavedTrackRow
                track={entry.track}
                onPlay={() =>
                  void playCollection({
                    collection: { kind: 'history' },
                    startIndex: index,
                    shuffle: false,
                  })
                }
              />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

/** Most titles in Continue reading. */
const CONTINUE_READING = 12;

/** Manga and novels read lately, newest first (Mihon's History, as a row). */
const ContinueReading: FC = () => {
  const { t } = useTranslation();
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const entries: (MangaLibraryEntry & { medium: 'manga' | 'novel' })[] = [
    ...(manga.data ?? []).map((entry) => ({ ...entry, medium: 'manga' as const })),
    ...(novels.data ?? []).map((entry) => ({ ...entry, medium: 'novel' as const })),
  ]
    .filter((entry) => entry.lastReadAt !== null)
    .sort((a, b) => (b.lastReadAt ?? 0) - (a.lastReadAt ?? 0))
    .slice(0, CONTINUE_READING);
  if (entries.length === 0) return null;
  return (
    <section aria-labelledby="continue-reading" className="flex flex-col gap-2">
      <h2 id="continue-reading" className="text-lg font-semibold">
        {t('home.continueReading')}
      </h2>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
        {entries.map((entry) => (
          <li key={entry.titleId} className="w-28 shrink-0">
            <Link
              to={ROUTES[entry.medium].title}
              search={{ source: entry.sourceId, id: entry.remoteId }}
              aria-label={entry.title}
              className="flex flex-col gap-1.5"
            >
              <span className="relative">
                <MangaCover url={entry.coverUrl} className="w-full rounded-xl" />
                {entry.unread > 0 && (
                  <span className="bg-primary text-background absolute top-1.5 left-1.5 rounded-md px-1.5 text-xs font-bold">
                    {entry.unread}
                  </span>
                )}
              </span>
              <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
            </Link>
          </li>
        ))}
      </Carousel>
    </section>
  );
};

/**
 * Shown while there's nothing to continue: where to start. × hides it for good (M46.37); Edit
 * Home brings it back.
 */
const GetStarted: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const { save, isPending } = useSaveLayout();
  return (
    <section
      data-testid="get-started"
      className="bg-surface relative flex flex-col gap-3 rounded-3xl px-5 py-6"
    >
      <button
        type="button"
        aria-label={t('home.startClose')}
        title={t('home.startClose')}
        disabled={isPending}
        onClick={() =>
          save(
            'homeSections',
            layout.homeSections.map((item) =>
              item.id === 'getStarted' ? { ...item, visible: false } : item,
            ),
          )
        }
        className="text-muted hover:text-foreground hover:bg-background absolute top-3 right-3 grid size-9 place-items-center rounded-full"
      >
        <X aria-hidden size={18} />
      </button>
      <h2 className="pr-10 text-lg font-semibold">{t('home.startTitle')}</h2>
      <p className="text-muted text-sm">{t('home.startBody')}</p>
      <div className="flex flex-wrap gap-2">
        <Link
          to="/browse"
          search={{ tab: 'search' }}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-semibold"
        >
          <Search aria-hidden size={16} />
          {t('search.title')}
        </Link>
        <Link
          to="/library"
          className="bg-background flex items-center gap-2 rounded-full px-5 py-2.5 text-sm font-medium"
        >
          <Library aria-hidden size={16} />
          {t('library.title')}
        </Link>
      </div>
    </section>
  );
};

/** Popular now and Trending (M46.9): the charts that fit, as rows. */
const ChartSection: FC<{ which: 'popular' | 'trending' }> = ({ which }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const chart = pickChart(charts.data?.charts ?? [], which);
  if (!chart) return null;
  return (
    <ChartRow
      chart={chart}
      title={t(which === 'popular' ? 'discover.popularNow' : 'discover.trending')}
    />
  );
};

/** Made for You's mixes as a row (M46.20); hidden until there are some. */
const MadeForYouRow: FC = () => {
  const { t } = useTranslation();
  const overview = useMadeForYou();
  const mixes = overview.data?.mixes ?? [];
  if (mixes.length === 0) return null;
  return (
    <section
      aria-label={t('forYou.title')}
      data-testid="made-for-you-row"
      className="flex flex-col gap-1"
    >
      <div className="flex items-baseline gap-3">
        <h2 className="text-lg font-semibold">
          <Link to="/music/for-you" className="hover:underline">
            {t('forYou.title')}
          </Link>
        </h2>
        <Link
          to="/music/for-you"
          className="text-muted hover:text-foreground ml-auto shrink-0 text-sm font-semibold"
        >
          {t('forYou.seeAll')}
        </Link>
      </div>
      <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
        {mixes.slice(0, 12).map((mix) => (
          <MixCard key={mix.id} mix={mix} />
        ))}
      </Carousel>
    </section>
  );
};

/** Home's sections by their layout id (M46.5; more rows from Browse all's kinds in M46.47). */
const HOME_SECTIONS: Record<string, FC> = {
  continueWatching: () => <ContinueWatchingRow />,
  continueReading: () => <ContinueReading />,
  madeForYou: () => <MadeForYouRow />,
  yourMusic: () => <YourMusic />,
  recentlyPlayed: () => <RecentlyPlayed />,
  popularMusic: () => <ChartSection which="popular" />,
  trendingMusic: () => <ChartSection which="trending" />,
  newReleases: () => <ChartGroupSection group="new" />,
  popularVideos: () => <PopularTitlesSection medium="video" />,
  popularComics: () => <PopularTitlesSection medium="manga" />,
  popularBooks: () => <PopularTitlesSection medium="novel" />,
  popularAudiobooks: () => <AudiobooksSection />,
  charts: () => <ChartTilesSection />,
  viralMusic: () => <ChartGroupSection group="viral" />,
  genres: () => <GenresSection />,
  decades: () => <DecadesSection />,
  undergroundMusic: () => <ChartGroupSection group="underground" />,
};

export const HomePage: FC = () => {
  const { t } = useTranslation();
  const desktop = useIsDesktop();
  const incognitoItem = useIncognitoItem();
  const layout = useLayout();
  const watching = useContinueWatching();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const nothingYet =
    watching.data?.length === 0 &&
    history.data?.length === 0 &&
    !manga.data?.some((entry) => entry.lastReadAt !== null) &&
    !novels.data?.some((entry) => entry.lastReadAt !== null);
  return (
    <section data-testid="home-page" className="flex flex-col gap-7">
      <TopBar
        title={t('home.heading')}
        actions={
          <>
            {/* The desktop bar has its own (M46.24). */}
            {!desktop && (
              <MascotButton
                className="hover:bg-surface grid size-11 place-items-center rounded-full"
                iconSize={22}
              />
            )}
            <TopBarAction
              label={t('search.title')}
              icon={Search}
              to="/browse"
              search={{ tab: 'search' }}
            />
            <OverflowMenu
              items={[
                incognitoItem,
                {
                  label: t('layout.editHome'),
                  icon: LayoutList,
                  to: '/settings/layout/homeSections',
                },
              ]}
            />
          </>
        }
      />
      <UpdateBanner />
      {layout.homeSections
        .filter((item) => item.visible)
        .map((item) => {
          if (item.id === 'getStarted') return nothingYet ? <GetStarted key={item.id} /> : null;
          const Section = HOME_SECTIONS[item.id];
          return Section ? <Section key={item.id} /> : null;
        })}
    </section>
  );
};
