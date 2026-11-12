import type { PodcastEpisode } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { Play } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { CardMenu } from '../shell/CardMenu';
import { CARD_MENU_BESIDE } from '../manga/CardPlayButton';
import { artistCover } from '../library/coverTargets';
import { useAlbumMenuItems, useArtistMenuItems, usePodcastMenuItems } from '../music/cardMenus';
import { useReleaseNews } from '../music/follows';
import { feedKey, pageKey, useHidden } from '../music/hidden';
import { useMadeForYou } from '../music/useMadeForYou';
import { formatLength } from '../podcasts/formatLength';
import { PodcastArt } from '../podcasts/PodcastArt';
import { episodeMenu } from '../podcasts/podcastMenus';
import {
  useDownloadEpisode,
  useLatestEpisodes,
  useOpenFeed,
  usePlayEpisode,
  useRemoveEpisodeDownload,
  useSetPlayed,
  useSubscriptions,
  useTopPodcasts,
} from '../podcasts/usePodcasts';
import { AlbumTile } from '../search/MusicFinds';
import { SkeletonSwap } from '../shell/SkeletonSwap';
import { useInView } from '../shell/useInView';
import { HomeCarousel, HomeSection, RowSkeleton } from './HomeSection';

/** Cards in these rows, at most. */
const SHOWN = 16;
/** Artists in Your top artists: each picture may be looked up in the sources. */
const TOP_ARTISTS = 12;

/** The artists played most now (Spotify's Your top artists), from Made for You's numbers. */
export const TopArtistsSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const overview = useMadeForYou();
  const hidden = useHidden();
  const menu = useArtistMenuItems();
  const artists = (overview.data?.stats.topArtists ?? []).filter(
    (name) => !hidden.hidesArtist(name),
  );
  if (artists.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('home.topArtists')}
      all={{ to: '/music/for-you' }}
      testId="home-top-artists"
    >
      <HomeCarousel>
        {artists.slice(0, TOP_ARTISTS).map((name) => (
          <li key={name} className="w-28 shrink-0 sm:w-32">
            <Link
              to="/artist"
              search={{ name }}
              className="relative flex flex-col items-center gap-1.5 text-center"
            >
              <CardMenu
                label={t('menu.options', { name })}
                items={() => menu({ name, imageUrl: null })}
              />
              <Artwork
                url={null}
                fallback={artistCover(name)}
                className="w-full rounded-full"
                iconSize={28}
              />
              <span className="w-full truncate text-sm font-medium">{name}</span>
              <span className="text-muted -mt-1 text-xs">{t('artist.kind')}</span>
            </Link>
          </li>
        ))}
      </HomeCarousel>
    </HomeSection>
  );
};

const EpisodeCard: FC<{ episode: PodcastEpisode }> = ({ episode }) => {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const play = usePlayEpisode();
  const setPlayed = useSetPlayed();
  const download = useDownloadEpisode();
  const removeDownload = useRemoveEpisodeDownload();
  const duration = episode.durationMs ?? 0;
  const position = episode.positionMs ?? 0;
  const left = position > 0 && duration > position ? duration - position : 0;
  const when = episode.publishedAt
    ? new Intl.DateTimeFormat(i18n.language, { month: 'short', day: 'numeric' }).format(
        episode.publishedAt,
      )
    : null;
  return (
    <li className="relative w-36 shrink-0 sm:w-40">
      <CardMenu
        label={t('menu.options', { name: episode.title })}
        items={() =>
          episodeMenu(episode, t, {
            play: () => play.mutate(episode.id),
            setPlayed: (played) => setPlayed.mutate({ id: episode.id, played }),
            download: () => download.mutate(episode.id),
            removeDownload: () => removeDownload.mutate(episode.id),
            openPodcast: () => void navigate({ to: '/podcast', search: { id: episode.podcastId } }),
          })
        }
        className={CARD_MENU_BESIDE}
      />
      <button
        type="button"
        onClick={() => play.mutate(episode.id)}
        aria-label={t('home.playEpisode', { title: episode.title, show: episode.podcastTitle })}
        data-testid="episode-card"
        className="group hover:bg-surface flex w-full flex-col gap-2 rounded-xl p-2 text-left"
      >
        <span className="relative block">
          <PodcastArt url={episode.artworkUrl} className="w-full rounded-lg shadow-lg" />
          <span className="bg-primary text-background absolute right-2 bottom-2 grid size-10 place-items-center rounded-full opacity-0 shadow-lg transition group-hover:opacity-100 group-focus-visible:opacity-100">
            <Play aria-hidden size={18} fill="currentColor" />
          </span>
          {left > 0 && duration > 0 && (
            <span aria-hidden className="absolute inset-x-2 bottom-1 h-1 rounded-full bg-black/40">
              <span
                className="bg-primary block h-full rounded-full"
                style={{ width: `${Math.min(100, (position / duration) * 100)}%` }}
              />
            </span>
          )}
        </span>
        <span className="line-clamp-2 text-sm font-semibold">{episode.title}</span>
        <span className="text-muted -mt-1 truncate text-xs">
          {[
            episode.podcastTitle,
            left > 0
              ? t('podcasts.left', { time: formatLength(left, t) })
              : duration > 0
                ? formatLength(duration, t)
                : when,
          ]
            .filter(Boolean)
            .join(' · ')}
        </span>
      </button>
    </li>
  );
};

/** New episodes of followed shows (Spotify's New episodes), not yet heard. */
export const NewEpisodesSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const latest = useLatestEpisodes();
  const hidden = useHidden();
  const episodes = (latest.data ?? []).filter(
    (episode) =>
      !episode.played && !hidden.hidesItem(pageKey('/podcast', { id: episode.podcastId })),
  );
  if (episodes.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('home.newEpisodes')}
      all={{ to: '/library', search: { view: 'podcasts' } }}
      testId="home-new-episodes"
    >
      <HomeCarousel tight>
        {episodes.slice(0, SHOWN).map((episode) => (
          <EpisodeCard key={episode.id} episode={episode} />
        ))}
      </HomeCarousel>
    </HomeSection>
  );
};

/**
 * Popular podcasts from the directories. What's cached shows at once; the directory is asked
 * once the row comes near the window. Shows marked Not interested stay out.
 */
export const PopularPodcastsSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLDivElement>();
  const navigate = useNavigate();
  const top = useTopPodcasts(seen);
  const followed = useSubscriptions();
  const open = useOpenFeed();
  const hidden = useHidden();
  const menu = usePodcastMenuItems();
  const following = new Map((followed.data ?? []).map((show) => [show.feedUrl, show.id]));
  const shows = (top.data ?? []).filter((show) => !hidden.hidesItem(feedKey(show.feedUrl)));
  if (shows.length === 0 && (top.isSuccess || top.isError)) return <div ref={ref} />;
  return (
    <div ref={ref}>
      <HomeSection
        id={id}
        title={t('home.popularPodcasts')}
        all={{ to: '/browse', search: { tab: 'search', type: 'music', view: 'podcasts' } }}
        testId="home-popular-podcasts"
      >
        <SkeletonSwap ready={shows.length > 0} skeleton={<RowSkeleton card="w-28 sm:w-32" />}>
          <HomeCarousel>
            {shows.slice(0, SHOWN).map((show) => (
              <li key={show.feedUrl} className="relative w-28 shrink-0 sm:w-32">
                <CardMenu
                  label={t('menu.options', { name: show.title })}
                  items={() =>
                    menu(
                      {
                        title: show.title,
                        author: show.author,
                        feedUrl: show.feedUrl,
                        artworkUrl: show.artworkUrl,
                        id: following.get(show.feedUrl) ?? null,
                        subscribed: following.has(show.feedUrl),
                      },
                      { notInterested: true },
                    )
                  }
                />
                <button
                  type="button"
                  disabled={open.isPending}
                  onClick={() =>
                    open.mutate(show.feedUrl, {
                      onSuccess: (view) =>
                        void navigate({ to: '/podcast', search: { id: view.id } }),
                    })
                  }
                  className="flex w-full flex-col gap-1.5 text-left"
                >
                  <PodcastArt url={show.artworkUrl} className="w-full rounded-lg" />
                  <span className="line-clamp-2 text-sm font-medium">{show.title}</span>
                  {show.author && (
                    <span className="text-muted -mt-1 truncate text-xs">{show.author}</span>
                  )}
                </button>
              </li>
            ))}
          </HomeCarousel>
        </SkeletonSwap>
      </HomeSection>
    </div>
  );
};

/** New albums and singles from followed artists (M46.42), as a row. */
export const FollowedReleasesSection: FC<{ id: string }> = ({ id }) => {
  const { t } = useTranslation();
  const news = useReleaseNews();
  const hidden = useHidden();
  const menu = useAlbumMenuItems();
  const albums = (news.data ?? []).filter(
    (item) =>
      !hidden.hidesArtist(item.artist) &&
      !hidden.hidesItem(
        pageKey('/music/album', { provider: item.album.provider.id, id: item.album.id }),
      ),
  );
  if (albums.length === 0) return null;
  return (
    <HomeSection
      id={id}
      title={t('home.followedReleases')}
      all={{ to: '/updates', search: { tab: 'releases' } }}
      testId="home-followed-releases"
    >
      <HomeCarousel tight>
        {albums.slice(0, SHOWN).map((item) => (
          <li key={`${item.album.provider.id}:${item.album.id}`} className="w-36 shrink-0 sm:w-40">
            <AlbumTile
              album={item.album}
              detail={item.artist}
              menu={() => menu(item.album, { notInterested: true })}
            />
          </li>
        ))}
      </HomeCarousel>
    </HomeSection>
  );
};
