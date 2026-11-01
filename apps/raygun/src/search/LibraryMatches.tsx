import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Clapperboard } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { TrackRow } from '../library/TrackRow';
import { playFromLibrary } from '../library/useLibrary';
import { MangaGrid } from '../manga/MangaGrid';
import { useMangaLibrary } from '../manga/useManga';
import { PodcastArt } from '../podcasts/PodcastArt';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { useResumeVideo } from '../watch/useWatch';
import { useLibraryTrackSearch } from './useCatalogSearch';

const matches = (text: string | null | undefined, query: string) =>
  (text ?? '').toLocaleLowerCase().includes(query.toLocaleLowerCase());

const Heading: FC<{ title: string }> = ({ title }) => (
  <h3 className="text-muted text-sm font-medium">{title}</h3>
);

/**
 * What you already have that matches: library tracks, manga, novels, podcast subscriptions and
 * videos you watched. `limit` keeps each group short in the All view.
 */
export const LibraryMatches: FC<{ query: string; limit?: number }> = ({ query, limit = 50 }) => {
  const { t } = useTranslation();
  const q = query.trim();
  const tracks = useLibraryTrackSearch(q);
  const manga = useMangaLibrary('manga');
  const novels = useMangaLibrary('novel');
  const podcasts = useSubscriptions();
  const videos = useQuery({
    queryKey: ['watch', 'history'],
    queryFn: () => unwrapResult(commands.videoHistory()),
  });
  const resume = useResumeVideo();

  if (!q) return null;
  const trackHits = (tracks.data ?? []).slice(0, limit);
  const mangaHits = (manga.data ?? []).filter((entry) => matches(entry.title, q)).slice(0, limit);
  const novelHits = (novels.data ?? []).filter((entry) => matches(entry.title, q)).slice(0, limit);
  const podcastHits = (podcasts.data ?? [])
    .filter((podcast) => matches(podcast.title, q) || matches(podcast.author, q))
    .slice(0, limit);
  const videoHits = (videos.data ?? []).filter((video) => matches(video.title, q)).slice(0, limit);
  const total =
    trackHits.length + mangaHits.length + novelHits.length + podcastHits.length + videoHits.length;
  const loaded = !tracks.isFetching && manga.data && novels.data && podcasts.data && videos.data;

  const grid = (entries: typeof mangaHits) =>
    entries.map((entry) => ({
      sourceId: entry.sourceId,
      id: entry.remoteId,
      title: entry.title,
      coverUrl: entry.coverUrl,
      unread: entry.unread,
    }));

  return (
    <section data-testid="library-matches" className="flex flex-col gap-4">
      {loaded && total === 0 && (
        <p className="text-muted text-sm">{t('search.libraryNothing', { query: q })}</p>
      )}
      {trackHits.length > 0 && (
        <div className="flex flex-col gap-1">
          <Heading title={t('library.media.music')} />
          <ul className="flex flex-col">
            {trackHits.map((track) => (
              <li key={track.id}>
                <TrackRow
                  track={track}
                  onPlay={() =>
                    void playFromLibrary({
                      selection: { kind: 'allTracks' },
                      startTrackId: track.id,
                      shuffle: false,
                    })
                  }
                />
              </li>
            ))}
          </ul>
        </div>
      )}
      {mangaHits.length > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.manga')} />
          <MangaGrid medium="manga" items={grid(mangaHits)} />
        </div>
      )}
      {novelHits.length > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.novel')} />
          <MangaGrid medium="novel" items={grid(novelHits)} />
        </div>
      )}
      {podcastHits.length > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.podcast')} />
          <ul className="bg-surface flex flex-col rounded-xl">
            {podcastHits.map((podcast) => (
              <li key={podcast.id}>
                <Link
                  to="/podcast"
                  search={{ id: podcast.id }}
                  className="flex items-center gap-3 px-4 py-3"
                >
                  <PodcastArt url={podcast.artworkUrl} className="w-10 shrink-0 rounded-md" />
                  <span className="flex min-w-0 flex-col">
                    <span className="truncate font-medium">{podcast.title}</span>
                    {podcast.author && (
                      <span className="text-muted truncate text-xs">{podcast.author}</span>
                    )}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        </div>
      )}
      {videoHits.length > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.video')} />
          <ul className="bg-surface flex flex-col rounded-xl">
            {videoHits.map((video) => (
              <li key={video.key}>
                <button
                  type="button"
                  onClick={() => resume.mutate(video.key)}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left"
                >
                  <Clapperboard aria-hidden size={18} className="text-muted shrink-0" />
                  <span className="truncate font-medium">{video.title}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
};
