import type { MusicChart, OnlineTrack } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Play } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { fromOnlineTrack } from '../collections/useCollections';
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

/** Songs shown in a row; the chart's page has the rest. */
const ROW_SONGS = 20;

const SongCard: FC<{ song: OnlineTrack; songs: OnlineTrack[]; rank: number }> = ({
  song,
  songs,
  rank,
}) => {
  const { t } = useTranslation();
  const menu = useTrackMenu();
  const playing = usePlayingMatch()(song);
  const play = () => void playResults(songs, song);
  return (
    <li className="w-36 shrink-0 sm:w-40">
      <button
        type="button"
        onClick={play}
        onContextMenu={menu(fromOnlineTrack(song), { onPlay: play })}
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
        <span className="text-muted -mt-1.5 truncate text-xs">{song.artist}</span>
      </button>
    </li>
  );
};

/**
 * A chart as a row of songs (M46.9): Home's Popular now and Trending, and every chart in Browse.
 * It loads when it comes near the window; a song plays the chart from there.
 */
export const ChartRow: FC<{ chart: MusicChart; title?: string }> = ({ chart, title }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLElement>();
  const songs = useChartSongs(chart.provider.id, chart.id, seen);
  const heading = title ?? chart.name;
  const shown = (songs.data ?? []).slice(0, ROW_SONGS);
  return (
    <section ref={ref} aria-label={heading} data-testid="chart-row" className="flex flex-col gap-1">
      <div className="flex items-baseline gap-3">
        <h2 className="min-w-0 truncate text-lg font-semibold">
          <Link
            to="/music/chart"
            search={{ provider: chart.provider.id, id: chart.id }}
            className="hover:underline"
          >
            {heading}
          </Link>
        </h2>
        <span className="text-muted truncate text-xs">{chart.provider.name}</span>
        <Link
          to="/music/chart"
          search={{ provider: chart.provider.id, id: chart.id }}
          className="text-muted hover:text-foreground ml-auto shrink-0 text-sm font-semibold"
        >
          {t('discover.seeAll')}
        </Link>
      </div>
      {songs.error ? (
        <SourceIssue compact source={chart.provider.name} onRetry={() => void songs.refetch()}>
          {songs.error.message}
        </SourceIssue>
      ) : (
        <Carousel className="-mx-4 gap-1 px-2 lg:-mx-2">
          {shown.length > 0
            ? shown.map((song, index) => (
                <SongCard
                  key={`${song.title}-${index}`}
                  song={song}
                  songs={songs.data ?? []}
                  rank={index + 1}
                />
              ))
            : Array.from({ length: 6 }, (_, index) => (
                <li key={index} aria-hidden className="w-36 shrink-0 p-2 sm:w-40">
                  <span className="bg-surface block aspect-square animate-pulse rounded-lg" />
                </li>
              ))}
        </Carousel>
      )}
    </section>
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
      {songs.map((song, index) => (
        <SongCard key={`${song.title}-${index}`} song={song} songs={songs} rank={index + 1} />
      ))}
    </Carousel>
  </section>
);
