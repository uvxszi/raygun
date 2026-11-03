import type { MusicChart, OnlineTrack } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Play } from 'lucide-react';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { fromOnlineTrack } from '../collections/useCollections';
import { HomeCarousel, HomeSection, RowSkeleton } from '../home/HomeSection';
import { Artwork } from '../library/Artwork';
import { playResults } from '../search/useOnlineSearch';
import { useInView } from '../shell/useInView';
import { useChartSongs } from './useDiscovery';
import { useTrackMenu } from './useTrackMenu';
import { Carousel } from '../shell/Carousel';
import { NowPlayingBars } from '../player/NowPlayingBars';
import { usePlayingMatch } from '../player/usePlayingMatch';
import { SourceIssue } from '../shell/SourceIssue';
import { SongMarks } from './SongMarks';
import { useHidden } from './hidden';

/** Songs shown in a row; the chart's page has the rest. */
const ROW_SONGS = 20;

const CARD = 'w-36 shrink-0 sm:w-40';

const SongCard: FC<{
  song: OnlineTrack;
  rank: number;
  playing: 'playing' | 'paused' | null;
  onPlay: () => void;
  onMenu: (event: MouseEvent) => void;
}> = ({ song, rank, playing, onPlay, onMenu }) => {
  const { t } = useTranslation();
  return (
    <li className={CARD}>
      <button
        type="button"
        onClick={onPlay}
        onContextMenu={onMenu}
        aria-label={t('discover.playChart', { name: song.title })}
        data-testid="chart-song"
        className="group hover:bg-surface flex w-full flex-col gap-2 rounded-xl p-2 text-left"
      >
        <span className="relative block">
          <Artwork url={song.artworkUrl} className="w-full rounded-lg shadow-lg" iconSize={40} />
          <span className="bg-background/80 absolute top-1.5 left-1.5 flex items-center gap-1 rounded-md px-1.5 text-xs font-bold tabular-nums">
            {playing && <NowPlayingBars playing={playing === 'playing'} />}
            {rank}
          </span>
          <span className="bg-primary text-background absolute right-2 bottom-2 grid size-10 translate-y-1 place-items-center rounded-full opacity-0 shadow-lg transition group-hover:translate-y-0 group-hover:opacity-100 group-focus-visible:opacity-100">
            <Play aria-hidden size={18} fill="currentColor" />
          </span>
        </span>
        <span
          className={`flex min-w-0 items-center gap-1 text-sm font-semibold ${playing ? 'text-primary' : ''}`}
        >
          <span className="truncate">{song.title}</span>
          <SongMarks title={song.title} artist={song.artist} />
        </span>
        <span className="text-muted -mt-1 truncate text-xs">{song.artist}</span>
      </button>
    </li>
  );
};

/**
 * Song cards for `songs`, each playing the whole list from itself. The menu and the playing mark
 * are worked out once for the row, not per card. On Home (`home`), songs marked Not interested
 * or by hidden artists stay out, and the menu offers Not interested.
 */
const SongCards: FC<{ songs: OnlineTrack[]; home?: boolean; limit?: number }> = ({
  songs,
  home = false,
  limit,
}) => {
  const menu = useTrackMenu();
  const playing = usePlayingMatch();
  const hidden = useHidden();
  const shown = home ? songs.filter((song) => !hidden.hidesSong(song)) : songs;
  return (
    <>
      {shown.slice(0, limit ?? shown.length).map((song, index) => {
        const play = () => void playResults(shown, song);
        return (
          <SongCard
            key={`${song.title}-${index}`}
            song={song}
            rank={index + 1}
            playing={playing(song)}
            onPlay={play}
            onMenu={menu(fromOnlineTrack(song), { onPlay: play, notInterested: home })}
          />
        );
      })}
    </>
  );
};

/**
 * A chart as one of Home's rows (Popular now, Trending, New releases…): placeholders while the
 * chart list or its songs load (`chart` is null until the list comes), so the row keeps its place;
 * a chart that fails or has nothing left to show is quietly left out.
 */
export const HomeChartRow: FC<{ chart: MusicChart | null; title: string; homeId: string }> = ({
  chart,
  title,
  homeId,
}) => {
  const [ref, seen] = useInView<HTMLDivElement>();
  const songs = useChartSongs(chart?.provider.id ?? '', chart?.id ?? '', seen);
  const hidden = useHidden();
  const list = songs.data ?? [];
  const left = list.filter((song) => !hidden.hidesSong(song));
  if (songs.error || (songs.isSuccess && left.length === 0)) return <div ref={ref} />;
  return (
    <div ref={ref}>
      <HomeSection
        id={homeId}
        title={title}
        all={
          chart
            ? { to: '/music/chart', search: { provider: chart.provider.id, id: chart.id } }
            : undefined
        }
        source={chart?.provider.name}
        testId="chart-row"
      >
        {left.length > 0 ? (
          <HomeCarousel tight>
            <SongCards songs={list} home limit={ROW_SONGS} />
          </HomeCarousel>
        ) : (
          <RowSkeleton card={CARD} />
        )}
      </HomeSection>
    </div>
  );
};

/**
 * A chart as a row of songs (M46.9), as Browse lists every chart. It loads when it comes near
 * the window; a song plays the chart from there.
 */
export const ChartRow: FC<{ chart: MusicChart; title?: string }> = ({ chart, title }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLDivElement>();
  const songs = useChartSongs(chart.provider.id, chart.id, seen);
  const heading = title ?? chart.name;
  const all = { to: '/music/chart', search: { provider: chart.provider.id, id: chart.id } };
  const list = songs.data ?? [];
  if (songs.isSuccess && list.length === 0) return <div ref={ref} />;
  return (
    <div ref={ref}>
      <section aria-label={heading} data-testid="chart-row" className="flex flex-col gap-1">
        <div className="flex items-baseline gap-3">
          <h2 className="min-w-0 truncate text-lg font-semibold">
            <Link to={all.to} search={all.search} className="hover:underline">
              {heading}
            </Link>
          </h2>
          <span className="text-muted truncate text-xs">{chart.provider.name}</span>
          <Link
            to={all.to}
            search={all.search}
            className="text-muted hover:text-foreground ml-auto shrink-0 text-sm font-semibold"
          >
            {t('discover.seeAll')}
          </Link>
        </div>
        {songs.error ? (
          <SourceIssue compact source={chart.provider.name} onRetry={() => void songs.refetch()}>
            {songs.error.message}
          </SourceIssue>
        ) : list.length > 0 ? (
          <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
            <SongCards songs={list} limit={ROW_SONGS} />
          </Carousel>
        ) : (
          <RowSkeleton card={CARD} />
        )}
      </section>
    </div>
  );
};

/**
 * A source's songs for a genre or a mood (M46.19), as a row like a chart's: the whole row plays
 * from the song pressed.
 */
export const SongsRow: FC<{ heading: string; source: string; songs: OnlineTrack[] }> = ({
  heading,
  source,
  songs,
}) => (
  <section aria-label={heading} data-testid="tag-source-row" className="flex flex-col gap-1">
    <div className="flex items-baseline gap-3">
      <h2 className="min-w-0 truncate text-lg font-semibold">{heading}</h2>
      <span className="text-muted truncate text-xs">{source}</span>
    </div>
    <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
      <SongCards songs={songs} />
    </Carousel>
  </section>
);
