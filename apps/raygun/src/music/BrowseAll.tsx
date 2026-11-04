import type { MusicChart } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  AudioLines,
  BookAudio,
  BookOpen,
  Clapperboard,
  Globe,
  LibraryBig,
  Mic,
  Music,
  Sparkles,
  TrendingUp,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useHasProvider } from '../extensions/useExtensions';
import { NoSources } from '../search/NoSources';
import { searchLink } from '../search/searchTypes';
import {
  chartKey,
  tileColor,
  useMusicCharts,
  useTagSummary,
  useTagVocabulary,
} from './useDiscovery';

/** Genres shown before "Show all": the ones most people look for, then the user's and the charts'. */
const FEATURED_GENRES = [
  'Pop',
  'Hip-Hop',
  'Rock',
  'Electronic',
  'R&B',
  'Latin',
  'K-Pop',
  'Anime',
  'Indie',
  'Metal',
  'Jazz',
  'Classical',
  'Lo-fi',
  'Country',
  'Drum & Bass',
  'Breakcore',
  'Phonk',
  'Hyperpop',
  'House',
  'Soundtrack',
];

type TileProps = {
  title: string;
  to: string;
  search?: Record<string, unknown>;
  image?: string | null;
  icon?: LucideIcon;
  testId?: string;
  /** The list item's classes: its width in a carousel (Home's rows, M46.47). */
  className?: string;
};

/** A coloured tile with its title and a tilted picture in the corner (Spotify's Browse all). */
export const BrowseTile: FC<TileProps> = ({
  title,
  to,
  search,
  image,
  icon: Icon,
  testId,
  className,
}) => (
  <li className={className}>
    <Link
      to={to}
      search={search}
      data-testid={testId}
      style={{ backgroundColor: tileColor(title) }}
      className="relative block aspect-[16/10] overflow-hidden rounded-lg p-3 text-white transition hover:brightness-110 sm:p-4"
    >
      <span className="relative z-10 block max-w-[70%] text-lg leading-tight font-bold break-words sm:text-xl">
        {title}
      </span>
      {image ? (
        <img
          src={image}
          alt=""
          loading="lazy"
          className="absolute -right-3 -bottom-1 size-[42%] rotate-[25deg] rounded object-cover shadow-[0_2px_8px_rgba(0,0,0,0.45)]"
        />
      ) : (
        Icon && (
          <Icon
            aria-hidden
            className="absolute -right-2 -bottom-2 size-[40%] rotate-[25deg] opacity-80"
            strokeWidth={1.5}
          />
        )
      )}
    </Link>
  </li>
);

const Grid: FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section aria-label={title} className="flex flex-col gap-3">
    <h2 className="text-xl font-bold">{title}</h2>
    <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3 sm:gap-4 lg:grid-cols-4 2xl:grid-cols-6">
      {children}
    </ul>
  </section>
);

const chartTile = (chart: MusicChart) => (
  <BrowseTile
    key={chartKey(chart)}
    title={chart.name}
    to="/music/chart"
    search={{ provider: chart.provider.id, id: chart.id }}
    image={chart.artworkUrl}
    icon={TrendingUp}
    testId="chart-tile"
  />
);

/**
 * Browse → Browse all (M46.9, Spotify's): every kind of thing to find, the charts, the genres and
 * moods in the user's songs, then every genre and mood. Kinds open Search with nothing typed,
 * which shows what's popular in that kind. Without a chart source, the charts' tile gives way to
 * the way to one.
 */
export const BrowseAll: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const chartSources = useHasProvider('music.charts');
  const summary = useTagSummary();
  const vocabulary = useTagVocabulary();
  const [allTags, setAllTags] = useState(false);
  const all = charts.data?.charts ?? [];
  // Tiles for the main kinds of chart; genres and moods have tiles of their own below.
  const mainCharts = all.filter((chart) => chart.group !== 'genre' && chart.group !== 'mood');
  // A genre tile shows its chart's picture when a source has one.
  const genrePicture = new Map<string, string>();
  for (const chart of all) {
    if (chart.tag && chart.artworkUrl && !genrePicture.has(chart.tag)) {
      genrePicture.set(chart.tag, chart.artworkUrl);
    }
  }
  const yours = (summary.data ?? []).slice(0, 8);
  const tags = vocabulary.data ?? [];
  const moods = tags.filter((tag) => tag.kind === 'mood');
  const featured = new Set([...FEATURED_GENRES, ...genrePicture.keys()]);
  const genres = tags.filter((tag) => tag.kind === 'genre' && (allTags || featured.has(tag.name)));
  const kinds: { title: string; icon: LucideIcon; to: string; search?: Record<string, unknown> }[] =
    [
      { title: t('discover.madeForYou'), icon: Sparkles, to: '/music/for-you' },
      ...(chartSources === false
        ? []
        : [{ title: t('discover.music'), icon: Music, to: '/music/charts' }]),
      // Kinds open their Search pill with nothing typed: what's popular there.
      {
        title: t('discover.audiobooks'),
        icon: BookAudio,
        ...searchLink('music', 'audiobooks'),
      },
      { title: t('discover.podcasts'), icon: Mic, ...searchLink('music', 'podcasts') },
      { title: t('discover.comics'), icon: LibraryBig, ...searchLink('manga') },
      { title: t('discover.books'), icon: BookOpen, ...searchLink('novel') },
      { title: t('discover.videos'), icon: Clapperboard, ...searchLink('video') },
      { title: t('discover.websites'), icon: Globe, to: '/browse', search: { tab: 'sources' } },
    ];

  return (
    <div data-testid="browse-all" className="flex flex-col gap-8">
      <Grid title={t('discover.start')}>
        {kinds.map((kind) => (
          <BrowseTile key={kind.title} {...kind} testId="kind-tile" />
        ))}
      </Grid>
      {mainCharts.length > 0 && (
        <Grid title={t('discover.charts')}>{mainCharts.map(chartTile)}</Grid>
      )}
      {(chartSources === false ||
        (charts.isSuccess && all.length === 0 && charts.data.errors.length === 0)) && (
        <NoSources label={t('discover.noCharts')} />
      )}
      {yours.length > 0 && (
        <Grid title={t('discover.yourTags')}>
          {yours.map((tag) => (
            <BrowseTile
              key={tag.name}
              title={tag.name}
              to="/music/tag"
              search={{ name: tag.name }}
              image={genrePicture.get(tag.name)}
              icon={AudioLines}
            />
          ))}
        </Grid>
      )}
      <Grid title={t('discover.genres')}>
        {[...moods, ...genres].map((tag) => (
          <BrowseTile
            key={tag.name}
            title={tag.name}
            to="/music/tag"
            search={{ name: tag.name }}
            image={genrePicture.get(tag.name)}
            icon={AudioLines}
            testId="tag-tile"
          />
        ))}
      </Grid>
      {!allTags && tags.length > 0 && (
        <button
          type="button"
          onClick={() => setAllTags(true)}
          className="bg-surface self-center rounded-full px-5 py-2 text-sm font-semibold"
        >
          {t('discover.seeAll')}
        </button>
      )}
    </div>
  );
};
