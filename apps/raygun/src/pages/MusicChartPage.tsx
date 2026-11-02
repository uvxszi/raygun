import type { OnlineTrack } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { LoaderCircle, Play, Shuffle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { useChartSongs, useMusicCharts } from '../music/useDiscovery';
import { OnlineTrackRow } from '../search/OnlineTrackRow';
import { playResults } from '../search/useOnlineSearch';
import { TopBar } from '../shell/TopBar';

/** A chart's page (M46.9): its songs in order, played from any of them. */
export const MusicChartPage: FC = () => {
  const { t } = useTranslation();
  const { provider, id } = useSearch({ from: '/music/chart' });
  const charts = useMusicCharts();
  const chart = charts.data?.charts.find(
    (candidate) => candidate.provider.id === provider && candidate.id === id,
  );
  const songs = useChartSongs(provider, id);
  const list = songs.data ?? [];
  const play = (shuffle: boolean) => {
    if (list.length === 0) return;
    const start = shuffle ? list[Math.floor(Math.random() * list.length)] : list[0];
    void playResults(list, start as OnlineTrack);
  };
  const artwork = chart?.artworkUrl ?? list.find((song) => song.artworkUrl)?.artworkUrl ?? null;

  return (
    <section data-testid="music-chart-page" className="flex flex-col gap-6">
      <TopBar back fallback="/browse" plainTitle title={t('discover.chart')} />
      <header className="flex flex-col items-center gap-4 sm:flex-row sm:items-end sm:gap-6">
        <Artwork url={artwork} className="size-40 rounded-lg shadow-xl sm:size-48" iconSize={56} />
        <div className="flex min-w-0 flex-col items-center gap-2 sm:items-start">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {chart?.provider.name ?? t('discover.chart')}
          </span>
          <h1 className="text-center text-3xl font-black tracking-tight sm:text-left sm:text-5xl">
            {chart?.name ?? t('discover.chart')}
          </h1>
          {chart?.description && <p className="text-muted text-sm">{chart.description}</p>}
          {list.length > 0 && (
            <p className="text-muted text-sm">{t('discover.songs', { count: list.length })}</p>
          )}
        </div>
      </header>
      <div className="flex gap-3">
        <button
          type="button"
          disabled={list.length === 0}
          onClick={() => play(false)}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold disabled:opacity-50"
        >
          <Play aria-hidden size={18} fill="currentColor" />
          {t('library.playAll')}
        </button>
        <button
          type="button"
          disabled={list.length === 0}
          onClick={() => play(true)}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5 disabled:opacity-50"
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
      </div>
      {songs.isPending && (
        <LoaderCircle aria-label={t('discover.loading')} className="text-muted animate-spin" />
      )}
      {songs.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {songs.error.message}
        </p>
      )}
      <ol className="flex flex-col">
        {list.map((song, index) => (
          <li key={`${song.title}-${index}`} className="flex items-center gap-3">
            <span className="text-muted w-6 shrink-0 text-right text-sm tabular-nums">
              {index + 1}
            </span>
            <div className="min-w-0 flex-1">
              <OnlineTrackRow track={song} onPlay={(tapped) => void playResults(list, tapped)} />
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
};
