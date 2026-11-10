import { commands } from '@raygun/bindings';
import type { OnlineTrack, TrackInput, YouTubeLink } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { ExternalLink, Music, Play, Video } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ROUTES } from '../manga/medium';
import { useMangaSources } from '../manga/useManga';
import { DownloadSongButton } from '../music/DownloadSongButton';
import { NoSources } from '../search/NoSources';
import { usePlaylistFinds } from '../search/useMusicFinds';
import { playResults } from '../search/useOnlineSearch';
import { SourceIssue } from '../shell/SourceIssue';
import { YtDlpJobCard } from './YtDlpJobs';
import { useYouTubeLink, useYtDlpActions, useYtDlpJobs, useYtDlpStatus } from './useYtDlp';

const BUTTON =
  'flex items-center gap-1.5 rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50';

/** The video source whose pages open playlists and channels by these ids, when it's installed
 * (M46.45). Raygun ships none; without one the card says so and points to the extensions. */
const YOUTUBE_EXTENSION = 'dev.raygun.youtube';

const asTrack = (link: YouTubeLink): OnlineTrack => ({
  externalId: link.id,
  title: link.title ?? link.id,
  artist: link.author,
  album: null,
  durationMs: null,
  artworkUrl: link.thumbnailUrl,
  source: { kind: 'youTube', videoId: link.id },
});

/** The video's sound as a song to download into the app (M46.51: YouTube's own audio file). */
const asSong = (link: YouTubeLink): TrackInput => ({
  title: link.title ?? link.id,
  artist: link.author,
  album: null,
  durationMs: null,
  artworkUrl: link.thumbnailUrl,
  source: { kind: 'youTube', videoId: link.id },
});

/**
 * A YouTube address typed in Search (M46.47): the video with Play, Play as music, and on desktop
 * Download video or Download audio through yt-dlp, with the download's progress; elsewhere its
 * sound downloads as a song (M46.51). Playlists and channels open on an installed video source's
 * pages; a playlist also opens as songs when a music source reads it from its address (Nuclear's
 * YouTube Playlists plugin).
 */
export const YouTubeLinkCard: FC<{ query: string }> = ({ query }) => {
  const link = useYouTubeLink(query);
  if (!link.data) return null;
  return <LinkCard link={link.data} query={query} key={link.data.url} />;
};

const LinkCard: FC<{ link: YouTubeLink; query: string }> = ({ link, query }) => {
  const { t } = useTranslation();
  const status = useYtDlpStatus(link.kind === 'video');
  const jobs = useYtDlpJobs();
  const { download, installFfmpeg, cancelInstall } = useYtDlpActions();
  const sources = useMangaSources('video');
  const youtube = sources.data?.find((source) => source.extensionId === YOUTUBE_EXTENSION);
  // The same search the Playlists row asks, so it's answered once.
  const songLists = usePlaylistFinds(link.kind === 'playlist' && !youtube ? query : '', false);
  const songList = songLists.list[0];
  const title = link.title ?? link.id;
  const mine = (jobs.data ?? []).filter((job) => job.url === link.url).slice(0, 2);
  const supported = status.data?.supported ?? false;

  return (
    <section
      aria-label={t('youtube.link')}
      data-testid="youtube-link"
      className="bg-surface flex flex-col gap-3 rounded-2xl p-3"
    >
      <div className="flex gap-3">
        {link.thumbnailUrl && (
          <img
            src={link.thumbnailUrl}
            alt=""
            className="aspect-video w-36 shrink-0 rounded-lg object-cover sm:w-48"
          />
        )}
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t(`youtube.linkKind.${link.kind}`)}
          </span>
          <span className="line-clamp-2 font-semibold">{title}</span>
          {link.author && <span className="text-muted truncate text-sm">{link.author}</span>}
        </div>
      </div>
      {link.kind === 'video' ? (
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void commands.youtubePlay(link.id, title, link.author)}
            className={`${BUTTON} bg-primary text-background`}
          >
            <Play aria-hidden size={15} className="fill-current" />
            {t('youtube.play')}
          </button>
          <button
            type="button"
            onClick={() => void playResults([asTrack(link)], asTrack(link))}
            className={`${BUTTON} bg-background`}
          >
            <Music aria-hidden size={15} />
            {t('youtube.playAsMusic')}
          </button>
          {supported && (
            <>
              <button
                type="button"
                disabled={download.isPending}
                onClick={() => download.mutate({ url: link.url, title, kind: 'video' })}
                className={`${BUTTON} bg-background`}
              >
                <Video aria-hidden size={15} />
                {t('youtube.downloadVideo')}
              </button>
              <button
                type="button"
                disabled={download.isPending}
                onClick={() => download.mutate({ url: link.url, title, kind: 'audio' })}
                className={`${BUTTON} bg-background`}
              >
                <Music aria-hidden size={15} />
                {t('youtube.downloadAudio')}
              </button>
            </>
          )}
          {status.data && !supported && (
            <DownloadSongButton track={asSong(link)} className="self-center" />
          )}
        </div>
      ) : youtube ? (
        <Link
          to={ROUTES.video.title}
          search={{ source: youtube.id, id: `${link.kind}:${link.id}` }}
          className={`${BUTTON} bg-primary text-background w-fit`}
        >
          <ExternalLink aria-hidden size={15} />
          {t(`youtube.open.${link.kind}`)}
        </Link>
      ) : songList ? (
        <Link
          to="/music/playlist"
          search={{ provider: songList.provider.id, id: songList.id }}
          className={`${BUTTON} bg-primary text-background w-fit`}
        >
          <Music aria-hidden size={15} />
          {t('youtube.open.songs', { source: songList.provider.name })}
        </Link>
      ) : songLists.isFetching ? null : (
        <NoSources label={t('youtube.needsExtension')} />
      )}
      {supported && status.data && (
        <p className="text-muted text-xs">
          {status.data.version
            ? t('youtube.ytdlpVersion', { version: status.data.version })
            : t('youtube.ytdlpFetched')}
          {!status.data.ffmpeg && (
            <>
              {' '}
              {t('youtube.noFfmpeg')}{' '}
              <button
                type="button"
                disabled={installFfmpeg.isPending || status.data.installing}
                onClick={() => installFfmpeg.mutate()}
                className="text-primary font-semibold disabled:opacity-50"
              >
                {installFfmpeg.isPending ? t('youtube.gettingFfmpeg') : t('youtube.getFfmpeg')}
              </button>
              {(installFfmpeg.isPending || status.data.installing) && (
                <>
                  {' '}
                  <button
                    type="button"
                    aria-label={t('youtube.cancelFetch')}
                    disabled={cancelInstall.isPending}
                    onClick={() => cancelInstall.mutate()}
                    className="text-muted hover:text-foreground font-semibold disabled:opacity-50"
                  >
                    {t('youtube.cancel')}
                  </button>
                </>
              )}
            </>
          )}
        </p>
      )}
      {(download.error ?? installFfmpeg.error) && (
        <SourceIssue
          compact
          title={t(download.error ? 'youtube.downloadFailed' : 'youtube.ffmpegFailed')}
        >
          {(download.error ?? installFfmpeg.error)?.message}
        </SourceIssue>
      )}
      {mine.length > 0 && (
        <ul className="flex flex-col gap-2">
          {mine.map((job) => (
            <YtDlpJobCard key={job.id} job={job} />
          ))}
        </ul>
      )}
    </section>
  );
};
