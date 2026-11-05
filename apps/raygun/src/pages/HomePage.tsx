import { commands } from '@raygun/bindings';
import type { MangaLibraryEntry } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { BookAudio, Compass, Heart, LayoutList, Library, Mic2, Search, X } from 'lucide-react';
import type { FC, ReactNode } from 'react';
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
import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import { useHasProvider } from '../extensions/useExtensions';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import { useMangaLibrary } from '../manga/useManga';
import { useLayout, useSaveLayout } from '../layout/useLayout';
import { useContextMenuItems, useMixCardMenu, useTitleMenuItems } from '../music/cardMenus';
import { HomeChartRow } from '../music/ChartRow';
import { pageKey, useHidden } from '../music/hidden';
import { MixCard } from '../music/MixCard';
import { TrackMoreButton } from '../collections/TrackMoreButton';
import { pickChart, useMusicCharts } from '../music/useDiscovery';
import { useMadeForYou } from '../music/useMadeForYou';
import { useIncognitoItem } from '../privacy/useIncognito';
import { useSettings } from '../settings/useSettings';
import { HomeFixedTiles } from '../library/LocalFilesTile';
import { useFixedTileMenuItems } from '../library/useFixedTileMenu';
import { HomeCarousel, HomeSection } from '../home/HomeSection';
import { useHomeRowActions } from '../home/homeRows';
import {
  FollowedReleasesSection,
  NewEpisodesSection,
  PopularPodcastsSection,
  TopArtistsSection,
} from '../home/ListeningRows';
import {
  AudiobooksSection,
  ChartGroupSection,
  ChartTilesSection,
  DecadesSection,
  GenresSection,
  PopularTitlesSection,
} from '../home/MoreRows';
import { MascotButton } from '../shell/MascotButton';
import { SmartChipsButton } from '../shell/SmartChipsButton';
import { OverflowMenu } from '../shell/OverflowMenu';
import { useIsDesktop } from '../shell/useIsDesktop';
import { QuickAccessButton } from '../shell/QuickAccessButton';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { UpdateBanner } from '../updates/UpdateBanner';
import { ContinueWatchingRow } from '../watch/ContinueWatchingRow';
import { ContinueListening } from '../podcasts/ContinueListening';
import { useContinueWatching } from '../watch/useWatch';
import { CardMenu } from '../shell/CardMenu';
import { CARD_MENU_BESIDE, CARD_MENU_BESIDE_TIGHT } from '../manga/CardPlayButton';

/** Playlists shown as shortcuts next to Favorites. */
const HOME_PLAYLISTS = 5;

const TILE_CLASS = 'bg-surface flex w-32 shrink-0 flex-col gap-2 rounded-xl p-2 text-sm';

/** Recently played from (M46.37) and favorite songs shown in Your music. */
const RECENT_SHOWN = 12;
const FAVORITES_SHOWN = 12;

type SectionProps = { id: string };

/**
 * Your music (M46.37): Favorites, Downloaded and Local files, then what was played from lately
 * (albums, playlists, mixes, artists, charts, podcasts, books), then playlists not among them,
 * and a row of the latest favorite songs. Each tile has its kind's menu, and Remove from Your
 * music; hidden artists and tiles stay out.
 */
const YourMusic: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const playlists = usePlaylists();
  const playlistMenu = usePlaylistMenuItems();
  const contextMenu = useContextMenuItems();
  const recent = useQuery({
    queryKey: ['recentContexts'],
    queryFn: () => unwrapResult(commands.recentContexts()),
  });
  const favorites = useFavorites();
  const tileMenu = useFixedTileMenuItems();
  const hidden = useHidden();
  // Kept only while recently played is on (M46.41).
  const keepRecent = useSettings().data?.privacy.recentlyPlayed !== false;
  const contexts = (keepRecent ? (recent.data ?? []) : [])
    .filter(
      (context) =>
        !(context.kind === 'artist' && hidden.hidesArtist(context.name)) &&
        !hidden.hidesItem(pageKey(context.to, context.search)),
    )
    .slice(0, RECENT_SHOWN);
  const recentPlaylists = new Set(
    contexts.filter((context) => context.kind === 'playlist').map((context) => context.search.id),
  );
  const latest = [...(favorites.data ?? [])]
    .filter((track) => !hidden.hidesCredit(track.artist))
    .sort((a, b) => (b.addedAt ?? 0) - (a.addedAt ?? 0))
    .slice(0, FAVORITES_SHOWN);
  return (
    <HomeSection id={id} title={t('home.yourMusic')} all={{ to: '/library' }} testId="your-music">
      <div className="flex flex-col gap-2">
        <HomeCarousel>
          <li>
            <Link to="/library" search={{ view: 'favorites' }} className={`relative ${TILE_CLASS}`}>
              <CardMenu
                label={t('menu.options', { name: t('collections.favorites') })}
                items={() => tileMenu('favorites')}
                className={CARD_MENU_BESIDE}
              />
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
                className={`relative ${TILE_CLASS}`}
              >
                <CardMenu
                  label={t('menu.options', { name: context.name })}
                  items={() => contextMenu(context)}
                  className={CARD_MENU_BESIDE}
                />
                <Artwork
                  url={context.artworkUrl}
                  className={`w-full ${context.kind === 'artist' ? 'rounded-full' : 'rounded-lg'}`}
                  iconSize={32}
                />
                <span className="flex min-w-0 flex-col gap-0.5">
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
                  className={`relative ${TILE_CLASS}`}
                >
                  <CardMenu
                    label={t('menu.options', { name: playlist.name })}
                    items={() => playlistMenu(playlist)}
                    className={CARD_MENU_BESIDE}
                  />
                  <PlaylistCover
                    artworkUrls={playlist.artworkUrls}
                    coverUrl={playlist.coverUrl}
                    className="aspect-square w-full rounded-lg"
                  />
                  <span className="truncate font-medium">{playlist.name}</span>
                </Link>
              </li>
            ))}
        </HomeCarousel>
        {latest.length > 0 && (
          <>
            <div className="mt-2 flex items-center gap-3">
              <h3 className="text-muted text-sm font-semibold">{t('home.latestFavorites')}</h3>
              <Link
                to="/library"
                search={{ view: 'favorites' }}
                aria-label={t('home.showAllOf', { name: t('home.latestFavorites') })}
                className="text-muted hover:text-foreground ml-auto px-1 text-sm font-semibold"
              >
                {t('discover.seeAll')}
              </Link>
            </div>
            <HomeCarousel>
              {latest.map((track) => (
                <li key={track.trackKey} className="relative w-28 shrink-0">
                  {/* The same menu as anywhere a song is listed (M46.40). */}
                  <TrackMoreButton
                    track={fromSavedTrack(track)}
                    extras={{ favorite: true }}
                    className={CARD_MENU_BESIDE_TIGHT}
                    iconSize={16}
                  />
                  <button
                    type="button"
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
                    <span className="text-muted -mt-0.5 truncate text-xs">{track.artist}</span>
                  </button>
                </li>
              ))}
            </HomeCarousel>
          </>
        )}
      </div>
    </HomeSection>
  );
};

/** What was played last, as rows (hidden artists' songs and Not interested ones stay out). */
const RecentlyPlayed: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const history = useHistory(HOME_HISTORY_LIMIT);
  const clear = useClearHistory();
  const hidden = useHidden();
  // Listening history off (M46.41): nothing of it shows.
  const keep = useSettings().data?.privacy.history !== false;
  const entries = (history.data ?? [])
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => !hidden.hidesSong(entry.track));
  if (!keep || entries.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('home.recentlyPlayed')}
      testId="recently-played"
      extra={
        <button
          type="button"
          disabled={clear.isPending}
          onClick={() => clear.mutate()}
          className="text-muted hover:text-foreground px-1 text-sm font-semibold"
        >
          {t('home.clearHistory')}
        </button>
      }
    >
      <ul className="flex flex-col">
        {entries.map(({ entry, index }) => (
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
    </HomeSection>
  );
};

/** Most titles in Continue reading. */
const CONTINUE_READING = 12;

/** Manga and novels read lately, newest first (Mihon's History, as a row). */
const ContinueReading: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const menu = useTitleMenuItems();
  const entries: (MangaLibraryEntry & { medium: 'manga' | 'novel' })[] = [
    ...(manga.data ?? []).map((entry) => ({ ...entry, medium: 'manga' as const })),
    ...(novels.data ?? []).map((entry) => ({ ...entry, medium: 'novel' as const })),
  ]
    .filter((entry) => entry.lastReadAt !== null)
    .sort((a, b) => (b.lastReadAt ?? 0) - (a.lastReadAt ?? 0))
    .slice(0, CONTINUE_READING);
  if (entries.length === 0) return null;
  return (
    <HomeSection id={id} title={t('home.continueReading')} testId="continue-reading">
      <HomeCarousel>
        {entries.map((entry) => (
          <li key={entry.titleId} className="w-28 shrink-0">
            <Link
              to={ROUTES[entry.medium].title}
              search={{ source: entry.sourceId, id: entry.remoteId }}
              aria-label={entry.title}
              className="relative flex flex-col gap-1.5"
            >
              <CardMenu
                label={t('menu.options', { name: entry.title })}
                items={() =>
                  menu({
                    medium: entry.medium,
                    sourceId: entry.sourceId,
                    id: entry.remoteId,
                    title: entry.title,
                    coverUrl: entry.coverUrl,
                  })
                }
              />
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
      </HomeCarousel>
    </HomeSection>
  );
};

const StartChip: FC<{ to: string; search?: Record<string, unknown>; children: ReactNode }> = ({
  to,
  search,
  children,
}) => (
  <Link
    to={to}
    search={search as never}
    className="bg-background hover:bg-background/70 flex items-center gap-2 rounded-full px-4 py-2.5 text-sm font-medium"
  >
    {children}
  </Link>
);

/**
 * Shown while there's nothing to continue: where to start, as plain chips; a kind with no source
 * installed (podcasts, audiobooks) is left out. × hides it for good (M46.37); Edit Home brings it
 * back.
 */
const GetStarted: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const { save, isPending } = useSaveLayout();
  const podcasts = useHasProvider('podcast.directory') !== false;
  const audiobooks = useHasProvider('audiobook.directory') !== false;
  return (
    <section
      aria-label={t('home.startTitle')}
      data-testid="get-started"
      className="bg-surface relative flex flex-col gap-3 rounded-3xl px-5 py-5"
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
      <div className="flex flex-wrap gap-2">
        <Link
          to="/browse"
          search={{ tab: 'search' }}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-4 py-2.5 text-sm font-semibold"
        >
          <Search aria-hidden size={16} />
          {t('search.title')}
        </Link>
        <StartChip to="/browse" search={{ tab: 'discover' }}>
          <Compass aria-hidden size={16} />
          {t('discover.title')}
        </StartChip>
        {podcasts && (
          <StartChip to="/browse" search={{ tab: 'search', type: 'music', view: 'podcasts' }}>
            <Mic2 aria-hidden size={16} />
            {t('discover.podcasts')}
          </StartChip>
        )}
        {audiobooks && (
          <StartChip to="/audiobooks">
            <BookAudio aria-hidden size={16} />
            {t('discover.audiobooks')}
          </StartChip>
        )}
        <StartChip to="/library">
          <Library aria-hidden size={16} />
          {t('library.title')}
        </StartChip>
      </div>
    </section>
  );
};

/**
 * Popular now and Trending (M46.9): the charts that fit, as rows. While the chart list first
 * loads, the row keeps its place with placeholders.
 */
const ChartSection: FC<SectionProps & { which: 'popular' | 'trending' }> = ({ id, which }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const chart = pickChart(charts.data?.charts ?? [], which);
  if (!chart && !charts.isPending) return null;
  return (
    <HomeChartRow
      chart={chart}
      title={t(which === 'popular' ? 'discover.popularNow' : 'discover.trending')}
      homeId={id}
    />
  );
};

/** Made for You's mixes as a row (M46.20); hidden until there are some. */
const MadeForYouRow: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const overview = useMadeForYou();
  const hidden = useHidden();
  const mixMenu = useMixCardMenu({ notInterested: true });
  const mixes = (overview.data?.mixes ?? []).filter(
    (mix) => !hidden.hidesItem(pageKey('/music/mix', { id: mix.id })),
  );
  if (mixes.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('forYou.title')}
      all={{ to: '/music/for-you' }}
      testId="made-for-you-row"
    >
      <HomeCarousel tight>
        {mixes.slice(0, 12).map((mix) => (
          <MixCard key={mix.id} mix={mix} menu={mixMenu(mix)} />
        ))}
      </HomeCarousel>
    </HomeSection>
  );
};

/** Continue watching keeps its own heading; its ⋯ offers Hide. */
const ContinueWatching: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const rowMenu = useHomeRowActions();
  return (
    <div data-home-section={id}>
      <ContinueWatchingRow menu={() => rowMenu(id, t('watch.continue'))} />
    </div>
  );
};

/** Episodes and books started and not finished (M46.48). */
const ContinueListeningSection: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const rowMenu = useHomeRowActions();
  return (
    <div data-home-section={id}>
      <ContinueListening menu={() => rowMenu(id, t('podcasts.continueListening'))} />
    </div>
  );
};

/** Home's sections by their layout id (M46.5; more rows from Browse all's kinds in M46.47). */
const HOME_SECTIONS: Record<string, FC<SectionProps>> = {
  continueWatching: ContinueWatching,
  continueReading: ContinueReading,
  continueListening: ContinueListeningSection,
  madeForYou: MadeForYouRow,
  yourMusic: YourMusic,
  topArtists: TopArtistsSection,
  recentlyPlayed: RecentlyPlayed,
  newEpisodes: NewEpisodesSection,
  popularMusic: ({ id }) => <ChartSection id={id} which="popular" />,
  trendingMusic: ({ id }) => <ChartSection id={id} which="trending" />,
  newReleases: ({ id }) => <ChartGroupSection id={id} group="new" />,
  followedReleases: FollowedReleasesSection,
  popularVideos: ({ id }) => <PopularTitlesSection id={id} medium="video" />,
  popularComics: ({ id }) => <PopularTitlesSection id={id} medium="manga" />,
  popularBooks: ({ id }) => <PopularTitlesSection id={id} medium="novel" />,
  popularAudiobooks: AudiobooksSection,
  popularPodcasts: PopularPodcastsSection,
  charts: ChartTilesSection,
  viralMusic: ({ id }) => <ChartGroupSection id={id} group="viral" />,
  genres: GenresSection,
  decades: DecadesSection,
  undergroundMusic: ({ id }) => <ChartGroupSection id={id} group="underground" />,
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
              <>
                <MascotButton
                  className="hover:bg-surface grid size-11 place-items-center rounded-full"
                  iconSize={22}
                />
                <SmartChipsButton
                  className="hover:bg-surface grid size-11 place-items-center rounded-full"
                  iconSize={22}
                />
              </>
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
            <QuickAccessButton />
          </>
        }
      />
      <UpdateBanner />
      {layout.homeSections
        .filter((item) => item.visible)
        .map((item) => {
          if (item.id === 'getStarted') return nothingYet ? <GetStarted key={item.id} /> : null;
          const Section = HOME_SECTIONS[item.id];
          return Section ? <Section key={item.id} id={item.id} /> : null;
        })}
    </section>
  );
};
