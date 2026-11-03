import type { OnlineTrack, PodcastSearchResult } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { LoaderCircle, Play, UserRound } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { PodcastArt } from '../podcasts/PodcastArt';
import { PodcastResultList } from '../podcasts/PodcastResultList';
import { useOpenFeed, usePodcastSearch } from '../podcasts/usePodcasts';
import { Carousel } from '../shell/Carousel';
import { rankSongs, useAlbumFinds, useArtistFinds } from './useMusicFinds';
import { OnlineTrackRow } from './OnlineTrackRow';
import { titleScore } from './rankResults';
import { useSeenSongs } from './seenBefore';
import type { MusicView } from './searchTypes';
import { playResults, useOnlineSearch } from './useOnlineSearch';

type Top =
  | { kind: 'song'; score: number; track: OnlineTrack }
  | { kind: 'artist'; score: number; name: string; imageUrl: string | null }
  | { kind: 'podcast'; score: number; podcast: PodcastSearchResult };

const Part: FC<{ title: string; onMore: () => void; children: ReactNode }> = ({
  title,
  onMore,
  children,
}) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <header className="flex items-center justify-between">
        <h2 className="text-lg font-bold">{title}</h2>
        <button
          type="button"
          onClick={onMore}
          className="text-muted hover:text-foreground text-sm font-semibold"
        >
          {t('search.seeAll')}
        </button>
      </header>
      {children}
    </section>
  );
};

/**
 * Music's Relevant pill (M46.25): the best match of any kind, then a few songs, artists, albums
 * and podcasts, each with See all. It reuses each pill's own search, so switching costs nothing.
 */
export const RelevantMusic: FC<{ query: string; onView: (view: MusicView) => void }> = ({
  query,
  onView,
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const openFeed = useOpenFeed();
  const songs = useOnlineSearch(query);
  const artists = useArtistFinds(query);
  const albums = useAlbumFinds(query);
  const podcasts = usePodcastSearch(query);

  // Songs already seen (Home, charts, earlier searches) show until the sources answer (M46.47).
  const seen = useSeenSongs(query);
  const ranked = songs.data
    ? rankSongs(
        query,
        songs.data.top,
        songs.data.providers.filter((result) => !result.error),
      )
    : seen.map((track) => ({
        track,
        source: undefined,
        score: Math.max(
          titleScore(track.title, query),
          titleScore(`${track.artist ?? ''} ${track.title}`, query),
        ),
      }));
  const tracks = ranked.map((entry) => entry.track);

  // The top result: whichever name answers the query best; songs win a tie.
  const candidates: Top[] = [];
  if (ranked[0]) {
    candidates.push({
      kind: 'song',
      score: Math.min(100, ranked[0].score),
      track: ranked[0].track,
    });
  }
  // An artist named like the query wins when several of the songs found are theirs: "daft punk"
  // means the band, not a song called "Daft Punk" by someone else. The artist comes from the
  // artist search, else from those songs.
  const theirSongs = ranked.filter((entry) => titleScore(entry.track.artist ?? '', query) >= 100);
  const artist =
    artists.list[0] ??
    (theirSongs[0]?.track.artist
      ? { name: theirSongs[0].track.artist, imageUrl: null as string | null }
      : undefined);
  if (artist) {
    candidates.push({
      kind: 'artist',
      score: titleScore(artist.name, query) + (theirSongs.length >= 2 ? 5 : -1),
      name: artist.name,
      imageUrl: artist.imageUrl,
    });
  }
  const podcast = podcasts.data?.[0];
  if (podcast) {
    candidates.push({ kind: 'podcast', score: titleScore(podcast.title, query) - 2, podcast });
  }
  const top = [...candidates].sort((a, b) => b.score - a.score)[0];

  const searching = songs.isPending && artists.isPending && albums.isPending && podcasts.isPending;
  if (searching && ranked.length === 0) {
    return <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />;
  }
  const settled =
    !songs.isPending && !artists.isPending && !albums.isPending && !podcasts.isPending;
  if (settled && !top && albums.list.length === 0) {
    return <p className="text-muted text-sm">{t('search.noResults', { query })}</p>;
  }

  const openTop = () => {
    if (!top) return;
    if (top.kind === 'song') void playResults(tracks, top.track);
    if (top.kind === 'artist') void navigate({ to: '/artist', search: { name: top.name } });
    if (top.kind === 'podcast') {
      openFeed.mutate(top.podcast.feedUrl, {
        onSuccess: (view) => void navigate({ to: '/podcast', search: { id: view.id } }),
      });
    }
  };

  const topTitle = !top
    ? ''
    : top.kind === 'song'
      ? top.track.title
      : top.kind === 'artist'
        ? top.name
        : top.podcast.title;
  const topKind = !top
    ? ''
    : top.kind === 'song'
      ? [t('search.kind.song'), top.track.artist].filter(Boolean).join(' · ')
      : top.kind === 'artist'
        ? t('artist.kind')
        : [t('search.kind.podcast'), top.podcast.author].filter(Boolean).join(' · ');

  return (
    <div data-testid="relevant-music" className="flex flex-col gap-6">
      {songs.isPending && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <LoaderCircle aria-hidden size={14} className="animate-spin" />
          {t('search.seenWhileSearching')}
        </p>
      )}
      {top && (
        <section aria-label={t('search.topResult')} className="flex flex-col gap-2">
          <h2 className="text-lg font-bold">{t('search.topResult')}</h2>
          <button
            type="button"
            data-testid="top-result"
            onClick={openTop}
            className="bg-surface hover:bg-surface/70 group relative flex max-w-md items-center gap-4 rounded-2xl p-4 text-left"
          >
            {top.kind === 'song' && (
              <Artwork
                url={top.track.artworkUrl}
                className="size-24 shrink-0 rounded-lg"
                iconSize={32}
              />
            )}
            {top.kind === 'artist' &&
              (top.imageUrl ? (
                <Artwork
                  url={top.imageUrl}
                  className="size-24 shrink-0 rounded-full"
                  iconSize={32}
                />
              ) : (
                <span className="bg-background text-muted grid size-24 shrink-0 place-items-center rounded-full">
                  <UserRound aria-hidden size={40} />
                </span>
              ))}
            {top.kind === 'podcast' && (
              <PodcastArt url={top.podcast.artworkUrl} className="w-24 shrink-0 rounded-lg" />
            )}
            <span className="flex min-w-0 flex-col gap-1">
              <span className="line-clamp-2 text-2xl font-bold">{topTitle}</span>
              <span className="text-muted truncate text-sm">{topKind}</span>
            </span>
            {top.kind === 'song' && (
              <span className="bg-primary text-background absolute right-4 bottom-4 grid size-11 place-items-center rounded-full opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
                <Play aria-hidden size={20} className="fill-current" />
              </span>
            )}
          </button>
        </section>
      )}
      {ranked.length > 0 && (
        <Part title={t('search.music.songs')} onMore={() => onView('songs')}>
          <ul className="flex flex-col">
            {ranked.slice(0, 5).map((entry, index) => (
              <li key={`${entry.track.title}-${index}`}>
                <OnlineTrackRow
                  track={entry.track}
                  source={entry.source}
                  onPlay={(tapped) => void playResults(tracks, tapped)}
                />
              </li>
            ))}
          </ul>
        </Part>
      )}
      {artists.list.length > 0 && (
        <Part title={t('search.music.artists')} onMore={() => onView('artists')}>
          <Carousel>
            {artists.list.slice(0, 10).map((found) => (
              <li key={`${found.provider.id}:${found.id}`} className="w-32 shrink-0">
                <Link
                  to="/artist"
                  search={{ name: found.name }}
                  className="hover:bg-surface flex flex-col items-center gap-2 rounded-xl p-2 text-center"
                >
                  {found.imageUrl ? (
                    <Artwork url={found.imageUrl} className="size-28 rounded-full" iconSize={32} />
                  ) : (
                    <span className="bg-surface text-muted grid size-28 place-items-center rounded-full">
                      <UserRound aria-hidden size={36} />
                    </span>
                  )}
                  <span className="w-full truncate text-sm font-semibold">{found.name}</span>
                </Link>
              </li>
            ))}
          </Carousel>
        </Part>
      )}
      {albums.list.length > 0 && (
        <Part title={t('search.music.albums')} onMore={() => onView('albums')}>
          <Carousel>
            {albums.list.slice(0, 10).map((album) => (
              <li key={`${album.provider.id}:${album.id}`} className="w-36 shrink-0">
                <Link
                  to="/music/album"
                  search={{ provider: album.provider.id, id: album.id }}
                  className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
                >
                  <Artwork url={album.artworkUrl} className="w-full rounded-lg" iconSize={32} />
                  <span className="truncate text-sm font-semibold">{album.title}</span>
                  <span className="text-muted truncate text-xs">
                    {[album.year, album.artist].filter(Boolean).join(' · ')}
                  </span>
                </Link>
              </li>
            ))}
          </Carousel>
        </Part>
      )}
      {podcasts.data && podcasts.data.length > 0 && (
        <Part title={t('search.music.podcasts')} onMore={() => onView('podcasts')}>
          <PodcastResultList results={podcasts.data.slice(0, 12)} grid />
        </Part>
      )}
    </div>
  );
};
