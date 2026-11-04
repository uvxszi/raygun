import type { OnlineTrack, SavedTrack } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { LoaderCircle, Play } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SavedTrackRow } from '../collections/SavedTrackRow';
import { ChartRow, SongsRow } from '../music/ChartRow';
import {
  tileColor,
  useMusicCharts,
  useTagSongs,
  useTagSources,
  useTagVocabulary,
} from '../music/useDiscovery';
import { playResults } from '../search/useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';
import { TopBar } from '../shell/TopBar';
import { joinCount, useSongsLength } from '../music/collectionLength';
import { usePlayContext } from '../music/playContext';

const asOnline = (track: SavedTrack): OnlineTrack => ({
  externalId: null,
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  artworkUrl: track.artworkUrl,
  source: track.source,
});

/**
 * A genre's or a mood's page (M46.9), from a tile in Browse all or a song's tag: the sources'
 * charts for it, every source's songs for it (M46.19), then the user's own songs that have it.
 */
export const MusicTagPage: FC = () => {
  const { t } = useTranslation();
  const { name } = useSearch({ from: '/music/tag' });
  const vocabulary = useTagVocabulary();
  const charts = useMusicCharts();
  const songs = useTagSongs(name);
  const sources = useTagSources(name);
  usePlayContext(
    name
      ? {
          kind: 'tag',
          name,
          subtitle: null,
          artworkUrl: null,
          to: '/music/tag',
          search: { name },
        }
      : null,
  );
  const kind = vocabulary.data?.find((tag) => tag.name === name)?.kind ?? 'genre';
  const tagCharts = (charts.data?.charts ?? []).filter((chart) => chart.tag === name);
  const list = songs.data ?? [];
  const length = useSongsLength(list);
  const play = (index: number) => {
    const queue = list.map(asOnline);
    void playResults(queue, queue[index] as OnlineTrack);
  };

  return (
    <section data-testid="music-tag-page" className="flex flex-col gap-6">
      <TopBar
        back
        fallback="/browse"
        plainTitle
        title={t(`discover.${kind === 'mood' ? 'mood' : 'tag'}`)}
      />
      <header
        style={{ backgroundColor: tileColor(name) }}
        className="-mx-4 flex min-h-40 flex-col justify-end gap-1 px-6 py-6 text-white lg:-mx-10 lg:px-10"
      >
        <span className="text-xs font-semibold tracking-wide uppercase opacity-80">
          {t(`discover.${kind === 'mood' ? 'mood' : 'tag'}`)}
        </span>
        <h1 className="text-4xl font-black tracking-tight sm:text-6xl">{name}</h1>
      </header>
      {tagCharts.map((chart) => (
        <ChartRow
          key={`${chart.provider.id}:${chart.id}`}
          chart={chart}
          title={t('discover.tagCharts', { name: `${chart.provider.name} ${name}` })}
        />
      ))}
      {sources.isPending && (
        <LoaderCircle aria-label={t('discover.loading')} className="text-muted animate-spin" />
      )}
      {sources.data?.rows
        // A source's genre chart is already shown above.
        .filter((row) => !tagCharts.some((chart) => chart.provider.id === row.provider.id))
        .map((row) => (
          <SongsRow
            key={row.provider.id}
            heading={row.name}
            source={row.provider.name}
            songs={row.tracks}
          />
        ))}
      {sources.data?.failures.map((failure) => (
        <SourceIssue key={failure.source} compact source={failure.source}>
          {failure.error}
        </SourceIssue>
      ))}
      <section aria-label={t('discover.tagSongs', { name })} className="flex flex-col gap-2">
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold">{t('discover.tagSongs', { name })}</h2>
          {list.length > 0 && (
            <span className="text-muted text-sm">
              {joinCount(t('discover.songs', { count: list.length }), length)}
            </span>
          )}
          {list.length > 0 && (
            <button
              type="button"
              onClick={() => play(0)}
              className="bg-primary text-background ml-auto flex items-center gap-2 rounded-full px-4 py-1.5 text-sm font-semibold"
            >
              <Play aria-hidden size={14} fill="currentColor" />
              {t('library.playAll')}
            </button>
          )}
        </div>
        {songs.isSuccess && list.length === 0 && (
          <p className="text-muted text-sm">{t('discover.tagNoSongs', { name })}</p>
        )}
        <ul className="flex flex-col">
          {list.map((track, index) => (
            <li key={`${track.trackKey}-${index}`}>
              <SavedTrackRow track={track} onPlay={() => play(index)} />
            </li>
          ))}
        </ul>
      </section>
    </section>
  );
};
