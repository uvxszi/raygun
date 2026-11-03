import type { ChartGroup, MangaCard } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { CalendarClock, TrendingUp } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { BookCard } from '../audiobooks/BookRow';
import { useBookDirectory, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { BrowseTile } from '../music/BrowseAll';
import { ChartRow } from '../music/ChartRow';
import { chartKey, useMusicCharts, useTagSummary, useTagVocabulary } from '../music/useDiscovery';
import { useSourceHealth } from '../search/sourceHealth';
import { sourceSearchOptions } from '../search/useCatalogSearch';
import { Carousel } from '../shell/Carousel';
import { useInView } from '../shell/useInView';

/** Tiles in Home's genre and decade rows. */
const TILES = 16;
/** Titles in a row of popular comics, books or videos. */
const TITLES = 18;
/** Sources asked for such a row: a few, so the row comes quickly. */
const HOME_SOURCES = 4;

const TILE_CLASS = 'w-40 shrink-0 sm:w-48';

/** A Home row (M46.47): its heading with Show all, then a carousel. */
const Row: FC<{
  title: string;
  all: { to: string; search?: Record<string, unknown> };
  testId: string;
  children: ReactNode;
}> = ({ title, all, testId, children }) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} data-testid={testId} className="flex flex-col gap-1">
      <div className="flex items-baseline gap-3">
        <h2 className="min-w-0 truncate text-lg font-semibold">
          <Link to={all.to} search={all.search as never} className="hover:underline">
            {title}
          </Link>
        </h2>
        <Link
          to={all.to}
          search={all.search as never}
          className="text-muted hover:text-foreground ml-auto shrink-0 text-sm font-semibold"
        >
          {t('discover.seeAll')}
        </Link>
      </div>
      <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">{children}</Carousel>
    </section>
  );
};

/** The first chart of a kind (new releases, viral, underground) as a row of songs (M46.47). */
export const ChartGroupSection: FC<{ group: ChartGroup }> = ({ group }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const chart = (charts.data?.charts ?? []).find((each) => each.group === group);
  if (!chart) return null;
  return <ChartRow chart={chart} title={t(`discover.groups.${group}`)} />;
};

/** Genres and moods as Browse all's tiles (M46.47): the user's own first, then the charts'. */
export const GenresSection: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const summary = useTagSummary();
  const vocabulary = useTagVocabulary();
  const picture = new Map<string, string>();
  for (const chart of charts.data?.charts ?? []) {
    if (chart.tag && chart.artworkUrl && !picture.has(chart.tag)) {
      picture.set(chart.tag, chart.artworkUrl);
    }
  }
  const names = [
    ...(summary.data ?? []).map((tag) => tag.name),
    ...(vocabulary.data ?? [])
      .filter((tag) => tag.kind === 'mood' || picture.has(tag.name))
      .map((tag) => tag.name),
  ];
  const shown = [...new Set(names)].slice(0, TILES);
  if (shown.length === 0) return null;
  return (
    <Row title={t('discover.genres')} all={{ to: '/browse' }} testId="home-genres">
      {shown.map((name) => (
        <BrowseTile
          key={name}
          title={name}
          to="/music/tag"
          search={{ name }}
          image={picture.get(name)}
          className={TILE_CLASS}
        />
      ))}
    </Row>
  );
};

/** Charts by decade as Browse all's tiles (M46.47). */
export const DecadesSection: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const decades = (charts.data?.charts ?? []).filter((chart) => chart.group === 'decade');
  if (decades.length === 0) return null;
  return (
    <Row title={t('discover.groups.decade')} all={{ to: '/music/charts' }} testId="home-decades">
      {decades.slice(0, TILES).map((chart) => (
        <BrowseTile
          key={chartKey(chart)}
          title={chart.name}
          to="/music/chart"
          search={{ provider: chart.provider.id, id: chart.id }}
          image={chart.artworkUrl}
          icon={CalendarClock}
          className={TILE_CLASS}
        />
      ))}
    </Row>
  );
};

/** Other charts (top, trending…) as tiles, past the two shown as rows (M46.47). */
export const ChartTilesSection: FC = () => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const main = (charts.data?.charts ?? []).filter(
    (chart) => chart.group !== 'genre' && chart.group !== 'mood' && chart.group !== 'decade',
  );
  if (main.length === 0) return null;
  return (
    <Row title={t('discover.charts')} all={{ to: '/music/charts' }} testId="home-charts">
      {main.slice(0, TILES).map((chart) => (
        <BrowseTile
          key={chartKey(chart)}
          title={chart.name}
          to="/music/chart"
          search={{ provider: chart.provider.id, id: chart.id }}
          image={chart.artworkUrl}
          icon={TrendingUp}
          className={TILE_CLASS}
        />
      ))}
    </Row>
  );
};

/** Popular audiobooks from the audiobook sources (M46.47). */
export const AudiobooksSection: FC = () => {
  const [ref, seen] = useInView<HTMLDivElement>();
  return <div ref={ref}>{seen && <PopularBooks />}</div>;
};

const PopularBooks: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { mutate: open } = useOpenAudiobook();
  const books = useBookDirectory('popular');
  const list = books.data?.pages.flatMap((page) => page.books) ?? [];
  if (list.length === 0) return null;
  return (
    <Row title={t('home.popularAudiobooks')} all={{ to: '/audiobooks' }} testId="home-audiobooks">
      {list.slice(0, TITLES).map((book) => (
        <BookCard
          key={book.feedUrl}
          book={book}
          onOpen={(chosen) =>
            open(chosen.feedUrl, {
              onSuccess: (id) => void navigate({ to: '/audiobook', search: { id } }),
            })
          }
        />
      ))}
    </Row>
  );
};

/** The labels of the popular rows, by medium. */
const POPULAR_TITLE: Record<Medium, string> = {
  manga: 'home.popularComics',
  novel: 'home.popularBooks',
  video: 'home.popularVideos',
};

/**
 * Popular comics, books or videos (M46.47): a few working sources' popular pages, the same
 * requests (and cache) as Browse → Search with nothing typed, taken in turns so one source
 * doesn't fill the row. It asks once it comes near the window.
 */
export const PopularTitlesSection: FC<{ medium: Medium }> = ({ medium }) => {
  const [ref, seen] = useInView<HTMLDivElement>();
  return <div ref={ref}>{seen && <PopularTitles medium={medium} />}</div>;
};

const PopularTitles: FC<{ medium: Medium }> = ({ medium }) => {
  const { t } = useTranslation();
  const sources = useMangaSources(medium);
  const failures = useSourceHealth((state) => state.failures);
  const picked = (sources.data ?? [])
    .filter((source) => source.enabled && !source.searchPill && !(source.id in failures))
    .slice(0, HOME_SOURCES);
  const results = useQueries({
    queries: picked.map((source) => sourceSearchOptions(medium, source.id, '')),
  });
  const pages = results.map((result) => result.data?.items ?? []);
  const items: MangaCard[] = [];
  const longest = Math.max(0, ...pages.map((page) => page.length));
  for (let round = 0; round < longest; round += 1) {
    for (const page of pages) {
      const item = page[round];
      if (item) items.push(item);
    }
  }
  if (items.length === 0) return null;
  const video = medium === 'video';
  return (
    <Row
      title={t(POPULAR_TITLE[medium])}
      all={{ to: '/browse', search: { tab: 'search', type: medium } }}
      testId={`home-popular-${medium}`}
    >
      {items.slice(0, TITLES).map((item) => (
        <li key={`${item.sourceId}:${item.id}`} className="w-28 shrink-0 sm:w-32">
          <Link
            to={ROUTES[medium].title}
            search={{ source: item.sourceId, id: item.id }}
            aria-label={item.title}
            className="flex flex-col gap-1.5"
          >
            <MangaCover url={item.coverUrl} video={video} className="w-full rounded-xl" />
            <span className="line-clamp-2 text-sm font-medium">{item.title}</span>
          </Link>
        </li>
      ))}
    </Row>
  );
};
