import type { VideoLinkView, VideoServerView } from '@raygun/bindings';
import {
  CircleAlert,
  Copy,
  Download,
  ExternalLink,
  LoaderCircle,
  Play,
  Star,
  X,
} from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { OverflowMenu } from '../shell/OverflowMenu';
import { useDownloadSourceVideo } from './useVideoLibrary';
import { useVideoLinks, useVideoServers, useVideoSourcePlay } from './useVideoSources';
import { SourceIssue } from '../shell/SourceIssue';

/** Play a link, or save it to the device (M43). */
export type PickIntent = 'play' | 'download';

/** The episode being played, as the player shows it. */
export type ServerTarget = {
  sourceId: string;
  titleId: string;
  /** The stored episode's id. */
  episodeId: string;
  /** The source's id for it. */
  remoteId: string;
  heading: string;
};

type ServerState = 'ready' | 'resolving' | 'empty' | 'failed';
/** Resolved servers first, then the ones still resolving; empty and failed ones last. */
const ORDER: ServerState[] = ['ready', 'resolving', 'empty', 'failed'];

/** A torrent (Miru sources) needs a torrent app: Raygun lists it but doesn't play it. */
const isTorrent = (link: VideoLinkView) => link.format === 'torrent';

/** DASH can play but not be saved. */
const savable = (link: VideoLinkView) =>
  link.format !== 'dash' && !link.url.includes('.mpd') && !isTorrent(link);

/** Plays a link here, or in another app (`external`). */
type PlayLink = (link: VideoLinkView, external?: boolean) => void;

const LinkList: FC<{
  links: VideoLinkView[];
  intent: PickIntent;
  onPlay: PlayLink;
  onDownload: (link: VideoLinkView) => void;
  busy: boolean;
}> = ({ links, intent, onPlay, onDownload, busy }) => {
  const { t } = useTranslation();
  return (
    <ul className="flex flex-col gap-2">
      {links.map((link) => (
        <li key={link.url} className="flex items-stretch gap-2">
          <button
            type="button"
            disabled={busy || isTorrent(link) || (intent === 'download' && !savable(link))}
            onClick={() => (intent === 'download' ? onDownload(link) : onPlay(link))}
            className="bg-background flex min-w-0 flex-1 items-center gap-3 rounded-xl px-4 py-3 text-left disabled:opacity-60"
          >
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-sm font-medium wrap-anywhere">
                {link.label ?? (isTorrent(link) ? t('watch.torrent') : t('watch.video'))}
              </span>
              {isTorrent(link) && (
                <span className="text-muted text-xs">{t('watch.needsTorrentApp')}</span>
              )}
              {link.subtitles.length > 0 && (
                <span className="text-muted text-xs">
                  {t('watch.subtitleCount', { count: link.subtitles.length })}
                </span>
              )}
              {intent === 'download' && !savable(link) && !isTorrent(link) && (
                <span className="text-muted text-xs">{t('watch.cantDownload')}</span>
              )}
            </span>
            {link.preferred && (
              <span className="text-primary flex items-center gap-1 text-xs font-semibold">
                <Star aria-hidden size={14} />
                {t('watch.preferred')}
              </span>
            )}
            {intent === 'download' ? (
              <Download aria-hidden size={18} className="shrink-0" />
            ) : (
              <Play aria-hidden size={18} className="shrink-0" />
            )}
          </button>
          {intent === 'play' && savable(link) && (
            <button
              type="button"
              aria-label={t('watch.downloadLink', { name: link.label ?? t('watch.video') })}
              disabled={busy}
              onClick={() => onDownload(link)}
              className="bg-background text-muted grid w-12 shrink-0 place-items-center rounded-xl disabled:opacity-60"
            >
              <Download aria-hidden size={18} />
            </button>
          )}
          {intent === 'play' && (
            <div className="bg-background grid shrink-0 place-items-center rounded-xl">
              <OverflowMenu
                label={t('watch.linkActions', { name: link.label ?? t('watch.video') })}
                items={[
                  ...(isTorrent(link)
                    ? []
                    : [
                        {
                          label: t('watch.openElsewhere'),
                          icon: ExternalLink,
                          onSelect: () => onPlay(link, true),
                        },
                      ]),
                  {
                    label: t('watch.copyLink'),
                    icon: Copy,
                    onSelect: () => void navigator.clipboard?.writeText(link.url),
                  },
                ]}
              />
            </div>
          )}
        </li>
      ))}
    </ul>
  );
};

/** One server: its links when it came resolved, else asked for on its own. */
const ServerItem: FC<{
  sourceId: string;
  server: VideoServerView;
  intent: PickIntent;
  onPlay: PlayLink;
  onDownload: (link: VideoLinkView) => void;
  busy: boolean;
}> = ({ sourceId, server, intent, onPlay, onDownload, busy }) => {
  const { t } = useTranslation();
  const links = useVideoLinks(sourceId, server.id, server.links === null);
  const list = server.links ?? links.data;
  const state: ServerState = list
    ? list.length > 0
      ? 'ready'
      : 'empty'
    : links.isError
      ? 'failed'
      : 'resolving';
  return (
    <li
      aria-label={server.name}
      data-state={state}
      style={{ order: ORDER.indexOf(state) }}
      className="flex flex-col gap-2"
    >
      <h3 className="text-muted flex items-center gap-2 text-xs font-semibold tracking-wide uppercase">
        {server.name}
        {state === 'resolving' && (
          <LoaderCircle
            role="status"
            aria-label={t('watch.resolving')}
            size={14}
            className="animate-spin"
          />
        )}
      </h3>
      {state === 'empty' && <p className="text-muted text-sm">{t('watch.serverEmpty')}</p>}
      {state === 'failed' && (
        <p className="text-muted flex items-start gap-1.5 text-sm wrap-anywhere">
          <CircleAlert aria-hidden size={16} className="mt-0.5 shrink-0" />
          {t('watch.serverFailed', { message: links.error?.message ?? '' })}
        </p>
      )}
      {list && list.length > 0 && (
        <LinkList
          links={list}
          intent={intent}
          onPlay={onPlay}
          onDownload={onDownload}
          busy={busy}
        />
      )}
    </li>
  );
};

/**
 * An episode's servers, each resolving on its own and showing its links as they arrive. Each link
 * plays, or downloads from its own button; opened to download, picking a link saves it.
 */
export const ServerPicker: FC<{
  target: ServerTarget;
  intent?: PickIntent;
  onClose: () => void;
}> = ({ target, intent = 'play', onClose }) => {
  const { t } = useTranslation();
  const servers = useVideoServers(target.sourceId, target.remoteId);
  const play = useVideoSourcePlay();
  const download = useDownloadSourceVideo();

  const onPlay: PlayLink = (link, external = false) =>
    play.mutate(
      { titleId: target.titleId, episodeId: target.episodeId, link, external },
      { onSuccess: onClose },
    );
  const onDownload = (link: VideoLinkView) =>
    download.mutate(
      { titleId: target.titleId, episodeId: target.episodeId, link },
      { onSuccess: onClose },
    );
  const error = play.error ?? download.error;

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="servers-title"
        className="bg-surface flex max-h-[85vh] w-full max-w-md flex-col gap-3 rounded-t-2xl pt-5 pb-[calc(1rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
      >
        <div className="flex items-start justify-between gap-3 px-5">
          <div className="min-w-0">
            <h2 id="servers-title" className="text-lg font-semibold">
              {t(intent === 'download' ? 'watch.downloadTitle' : 'watch.servers')}
            </h2>
            <p className="text-muted line-clamp-1 text-sm">{target.heading}</p>
          </div>
          <button
            type="button"
            aria-label={t('watch.close')}
            onClick={onClose}
            className="bg-background grid size-9 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={18} />
          </button>
        </div>
        <div className="flex min-h-0 flex-col gap-4 overflow-y-auto px-5">
          {servers.isPending && (
            <LoaderCircle
              role="status"
              aria-label={t('watch.findingServers')}
              className="text-muted mx-auto animate-spin"
            />
          )}
          {servers.error && <SourceIssue>{servers.error.message}</SourceIssue>}
          {servers.data?.length === 0 && (
            <p className="text-muted text-sm">{t('watch.noServers')}</p>
          )}
          {servers.data && servers.data.length > 0 && (
            <ul className="flex flex-col gap-4">
              {servers.data.map((server) => (
                <ServerItem
                  key={server.id}
                  sourceId={target.sourceId}
                  server={server}
                  intent={intent}
                  onPlay={onPlay}
                  onDownload={onDownload}
                  busy={play.isPending || download.isPending}
                />
              ))}
            </ul>
          )}
          {error && <SourceIssue>{error.message}</SourceIssue>}
        </div>
      </div>
    </div>
  );
};
