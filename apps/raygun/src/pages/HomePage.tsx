import { Link } from '@tanstack/react-router';
import { BookAudio, Compass, LayoutList, Library, Mic2, Search, X } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { HOME_HISTORY_LIMIT, useHistory } from '../collections/useCollections';

import { useHasProvider } from '../extensions/useExtensions';
import { useMangaLibrary } from '../manga/useManga';
import { useLayout, useSaveLayout } from '../layout/useLayout';
import { useMixCardMenu } from '../music/cardMenus';
import { HomeChartRow } from '../music/ChartRow';
import { pageKey, useHidden } from '../music/hidden';
import { MixCard } from '../music/MixCard';
import { pickChart, useMusicCharts } from '../music/useDiscovery';
import { useMadeForYou } from '../music/useMadeForYou';
import { useOfflineItem } from '../network/useNetworkMode';
import { useIncognitoItem } from '../privacy/useIncognito';
import { HomeCarousel, HomeSection } from '../home/HomeSection';
import { useHomeRowActions } from '../home/homeRows';
import { QuickAccessSection } from '../home/QuickAccessSection';
import { ContinueReadingRow } from '../home/ContinueReadingRow';
import { RecentlyPlayedRow } from '../home/RecentlyPlayedRow';
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

type SectionProps = { id: string };

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
  // Empty, the wrapper isn't laid out: Home's gap would leave a blank band at the top.
  return (
    <div data-home-section={id} className="empty:hidden">
      <ContinueWatchingRow menu={() => rowMenu(id, t('watch.continue'))} />
    </div>
  );
};

/** Episodes and books started and not finished (M46.48). */
const ContinueListeningSection: FC<SectionProps> = ({ id }) => {
  const { t } = useTranslation();
  const rowMenu = useHomeRowActions();
  return (
    <div data-home-section={id} className="empty:hidden">
      <ContinueListening menu={() => rowMenu(id, t('podcasts.continueListening'))} />
    </div>
  );
};

/** Home's sections by their layout id (M46.5; more rows from Browse all's kinds in M46.47). */
const HOME_SECTIONS: Record<string, FC<SectionProps>> = {
  continueWatching: ContinueWatching,
  continueReading: ContinueReadingRow,
  continueListening: ContinueListeningSection,
  madeForYou: MadeForYouRow,
  quickAccess: QuickAccessSection,
  topArtists: TopArtistsSection,
  recentlyPlayed: RecentlyPlayedRow,
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
  const offlineItem = useOfflineItem();
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
                offlineItem,
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
