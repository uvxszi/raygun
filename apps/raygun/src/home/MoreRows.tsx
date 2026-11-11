import type { ChartGroup, MangaCard } from '@raygun/bindings';
import { useQueries } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { CalendarClock, TrendingUp } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { BookCard } from '../audiobooks/BookRow';
import { useBookDirectory, useOpenAudiobook } from '../audiobooks/useAudiobooks';
import { bookWords } from '../manga/bookWords';
import { MangaCover } from '../manga/MangaCover';
import { ROUTES } from '../manga/medium';
import type { Medium } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { BrowseTile } from '../music/BrowseAll';
import { titlePage, useBookMenuItems, useTitleMenuItems } from '../music/cardMenus';
import { HomeChartRow } from '../music/ChartRow';
import { feedKey, pageKey, useHidden } from '../music/hidden';
import { chartKey, useMusicCharts, useTagSummary, useTagVocabulary } from '../music/useDiscovery';
import { useSourceHealth } from '../search/sourceHealth';
import { sourceSearchOptions } from '../search/useCatalogSearch';
import { useInView } from '../shell/useInView';
import { HomeCarousel, HomeSection, RowSkeleton } from './HomeSection';
import { CARD_MENU_CLASS, CardPlayButton } from '../manga/CardPlayButton';
import { usePlayTitle } from '../manga/titleMenu';
import { MenuButton } from '../shell/MenuButton';

/** Tiles in Home's genre and decade rows. */
const TILES = 16;
/** Titles in a row of popular comics, books or videos. */
const TITLES = 18;
/** Sources asked for such a row: a few, so the row comes quickly. */
const HOME_SOURCES = 4;

const TILE_CLASS = 'w-40 shrink-0 sm:w-48';

/** The first chart of a kind (new releases, viral, underground) as a row of songs (M46.47). */
export const ChartGroupSection: FC<{ id: string; group: ChartGroup }> = ({ id, group }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const chart = (charts.data?.charts ?? []).find((each) => each.group === group);
  if (!chart) return null;
  return <HomeChartRow chart={chart} title={t(`discover.groups.${group}`)} homeId={id} />;
};

/** Genres and moods as Browse all's tiles (M46.47): the user's own first, then the charts'. */
export const GenresSection: FC<{ id: string }> = ({ id }) => {
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
    <HomeSection
      id={id}
      title={t('discover.genres')}
      all={{ to: '/browse', search: { tab: 'discover' } }}
      testId="home-genres"
    >
      <HomeCarousel>
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
      </HomeCarousel>
    </HomeSection>
  );
};

/** Charts by decade as Browse all's tiles (M46.47). */
export const DecadesSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const decades = (charts.data?.charts ?? []).filter((chart) => chart.group === 'decade');
  if (decades.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('discover.groups.decade')}
      all={{ to: '/music/charts' }}
      testId="home-decades"
    >
      <HomeCarousel>
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
      </HomeCarousel>
    </HomeSection>
  );
};

/** Other charts (top, trending…) as tiles, past the two shown as rows (M46.47). */
export const ChartTilesSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const charts = useMusicCharts();
  const main = (charts.data?.charts ?? []).filter(
    (chart) => chart.group !== 'genre' && chart.group !== 'mood' && chart.group !== 'decade',
  );
  if (main.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('discover.charts')}
      all={{ to: '/music/charts' }}
      testId="home-charts"
    >
      <HomeCarousel>
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
      </HomeCarousel>
    </HomeSection>
  );
};

/**
 * Popular audiobooks from the audiobook sources (M46.47). What's cached shows at once; the
 * sources are asked once the row comes near the window.
 */
export const AudiobooksSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLDivElement>();
  const navigate = useNavigate();
  const { mutate: open } = useOpenAudiobook();
  const books = useBookDirectory('popular', '', seen);
  const hidden = useHidden();
  const menu = useBookMenuItems();
  const list = (books.data?.pages.flatMap((page) => page.books) ?? []).filter(
    (book) => !hidden.hidesItem(feedKey(book.feedUrl)),
  );
  const settled = books.isError || (books.isSuccess && !books.isFetching);
  if (list.length === 0 && settled) return <div ref={ref} />;
  return (
    <div ref={ref}>
      <HomeSection
        id={id}
        title={t('home.popularAudiobooks')}
        all={{ to: '/audiobooks' }}
        testId="home-audiobooks"
      >
        {list.length > 0 ? (
          <HomeCarousel tight>
            {list.slice(0, TITLES).map((book) => (
              <BookCard
                key={book.feedUrl}
                book={book}
                menu={() =>
                  menu(
                    {
                      title: book.title,
                      author: book.author,
                      artworkUrl: book.artworkUrl,
                      feedUrl: book.feedUrl,
                    },
                    { notInterested: true },
                  )
                }
                onOpen={(chosen) =>
                  open(chosen.feedUrl, {
                    onSuccess: (bookId) =>
                      void navigate({ to: '/audiobook', search: { id: bookId } }),
                  })
                }
              />
            ))}
          </HomeCarousel>
        ) : (
          <RowSkeleton card="w-32 sm:w-40" />
        )}
      </HomeSection>
    </div>
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
 * doesn't fill the row. What's cached shows at once; the sources are asked once the row comes
 * near the window. Titles marked Not interested stay out.
 */
export const PopularTitlesSection: FC<{ id: string; medium: Medium }> = ({ id, medium }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLDivElement>();
  const sources = useMangaSources(medium);
  const failures = useSourceHealth((state) => state.failures);
  const hidden = useHidden();
  const menu = useTitleMenuItems();
  const play = usePlayTitle();
  const picked = (sources.data ?? [])
    .filter((source) => source.enabled && !source.searchPill && !(source.id in failures))
    .slice(0, HOME_SOURCES);
  const results = useQueries({
    queries: picked.map((source) => ({
      ...sourceSearchOptions(medium, source.id, ''),
      enabled: seen,
    })),
  });
  const pages = results.map((result) => result.data?.items ?? []);
  const items: MangaCard[] = [];
  const longest = Math.max(0, ...pages.map((page) => page.length));
  for (let round = 0; round < longest; round += 1) {
    for (const page of pages) {
      const item = page[round];
      if (
        item &&
        !hidden.hidesItem(pageKey(ROUTES[medium].title, { source: item.sourceId, id: item.id }))
      ) {
        items.push(item);
      }
    }
  }
  // Nothing to wait for: no sources, or every one has answered.
  const settled =
    sources.isSuccess &&
    (picked.length === 0 || results.every((result) => result.isSuccess || result.isError));
  if (items.length === 0 && (settled || sources.isError)) return <div ref={ref} />;
  const video = medium === 'video';
  const card = 'w-28 shrink-0 sm:w-32';
  return (
    <div ref={ref}>
      <HomeSection
        id={id}
        title={t(POPULAR_TITLE[medium])}
        all={{ to: '/browse', search: { tab: 'search', type: medium } }}
        testId={`home-popular-${medium}`}
      >
        {items.length > 0 ? (
          <HomeCarousel>
            {items.slice(0, TITLES).map((item) => {
              const title = {
                medium,
                sourceId: item.sourceId,
                id: item.id,
                title: item.title,
                coverUrl: item.coverUrl,
              };
              const page = titlePage(title);
              return (
                <li key={`${item.sourceId}:${item.id}`} className={card}>
                  <Link
                    to={page.to}
                    search={page.search}
                    aria-label={item.title}
                    className="flex flex-col gap-1.5"
                  >
                    <span className="relative">
                      <MangaCover
                        url={item.coverUrl}
                        video={video}
                        book={bookWords(medium, item.title)}
                        className="w-full rounded-xl"
                      />
                      {video && (
                        <CardPlayButton
                          label={t('menu.playTitle', { name: item.title })}
                          onPlay={() => play(title)}
                        />
                      )}
                      <MenuButton
                        label={t('menu.options', { name: item.title })}
                        items={() => menu(title, { notInterested: true })}
                        className={CARD_MENU_CLASS}
                        iconSize={16}
                      />
                    </span>
                    <span className="line-clamp-2 text-sm font-medium">{item.title}</span>
                  </Link>
                </li>
              );
            })}
          </HomeCarousel>
        ) : (
          <RowSkeleton card={card} shape="poster" />
        )}
      </HomeSection>
    </div>
  );
};
