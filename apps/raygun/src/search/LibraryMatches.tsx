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
import { useDrawnCount } from '../shell/useDrawnCount';
import { useResumeVideo } from '../watch/useWatch';
import { useLibraryTrackSearch } from './useCatalogSearch';

const matches = (text: string | null | undefined, query: string) =>
  (text ?? '').toLocaleLowerCase().includes(query.toLocaleLowerCase());

const Heading: FC<{ title: string }> = ({ title }) => (
  <h3 className="text-muted text-sm font-medium">{title}</h3>
);

/** Matches drawn at first, then as many more at a time (`useDrawnCount`). */
const FIRST_MATCHES = 24;

/** `budget` shared out over groups of `counts`, in order: how many of each to draw. */
const shareOut = (counts: number[], budget: number) => {
  let left = budget;
  return counts.map((count) => {
    const taken = Math.min(count, left);
    left -= taken;
    return taken;
  });
};

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
  const hits = <T,>(list: T[] | undefined, match: (item: T) => boolean) =>
    q ? (list ?? []).filter(match).slice(0, limit) : [];
  const trackHits = hits(tracks.data, () => true);
  const mangaHits = hits(manga.data, (entry) => matches(entry.title, q));
  const novelHits = hits(novels.data, (entry) => matches(entry.title, q));
  const podcastHits = hits(
    podcasts.data,
    (podcast) => matches(podcast.title, q) || matches(podcast.author, q),
  );
  const videoHits = hits(videos.data, (video) => matches(video.title, q));
  const counts = [trackHits, mangaHits, novelHits, podcastHits, videoHits].map(
    (group) => group.length,
  );
  const total = counts.reduce((sum, count) => sum + count, 0);
  // Many matches: the first at once, the rest as each part is on the screen; each group shows
  // its part of what's drawn, in order.
  const drawn = useDrawnCount(total, FIRST_MATCHES, FIRST_MATCHES, q);
  const [tracksShown = 0, mangaShown = 0, novelsShown = 0, podcastsShown = 0, videosShown = 0] =
    shareOut(counts, drawn);

  if (!q) return null;
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
      {tracksShown > 0 && (
        <div className="flex flex-col gap-1">
          <Heading title={t('library.media.music')} />
          <ul className="flex flex-col">
            {trackHits.slice(0, tracksShown).map((track) => (
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
      {mangaShown > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.manga')} />
          <MangaGrid medium="manga" items={grid(mangaHits.slice(0, mangaShown))} />
        </div>
      )}
      {novelsShown > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.novel')} />
          <MangaGrid medium="novel" items={grid(novelHits.slice(0, novelsShown))} />
        </div>
      )}
      {podcastsShown > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.podcast')} />
          <ul className="bg-surface flex flex-col rounded-xl">
            {podcastHits.slice(0, podcastsShown).map((podcast) => (
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
      {videosShown > 0 && (
        <div className="flex flex-col gap-2">
          <Heading title={t('library.media.video')} />
          <ul className="bg-surface flex flex-col rounded-xl">
            {videoHits.slice(0, videosShown).map((video) => (
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
