import type { SourceRank, StremioStream, StremioStreamGroup } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import {
  ChevronDown,
  Clapperboard,
  Copy,
  Download,
  ExternalLink,
  History,
  LoaderCircle,
  Magnet,
  Play,
  X,
} from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { useBackLayer } from '../shell/backLayers';
import { OverflowMenu } from '../shell/OverflowMenu';
import type { OverflowItem } from '../shell/OverflowMenu';
import { useIsDesktop } from '../shell/useIsDesktop';
import { formatBytes } from './episodeLabel';
import type { PickIntent } from './ServerPicker';
import { inRankOrder, streamLabel, useSourceOrder } from './useSourceOrder';
import { useOpenStremioLink, useStremioPlay, useStremioStreams } from './useStremio';
import { SubtitleChoice } from './SubtitleChoice';
import { useDownloadStremio } from './useVideoLibrary';
import { SourceIssue } from '../shell/SourceIssue';

/** What is being played, as the player shows it. */
export type StreamTarget = {
  kind: string;
  videoId: string;
  title: string;
  subtitle: string | null;
  poster: string | null;
  /** The saved title, so a download counts toward it in the library. */
  titleId?: string | null;
  /** The movie or series, whose details say which ids its streams go by, and whose remembered
   * speed, aspect and tracks the player uses. */
  metaId: string;
};

/** Direct streams that aren't DASH can be saved, and torrents while they play here. */
const savable = (stream: StremioStream, torrents: boolean) =>
  (stream.kind === 'direct' && !stream.target.toLowerCase().includes('.mpd')) ||
  (stream.kind === 'torrent' && torrents);

/** Plays here, or opens something here (another title, a web page); torrents once turned on. */
const usableHere = (stream: StremioStream, torrents: boolean) =>
  stream.kind === 'direct' ||
  stream.kind === 'youTube' ||
  stream.kind === 'external' ||
  stream.kind === 'title' ||
  (stream.kind === 'torrent' && torrents);

/** Needs another app: Stremio's own streaming server, or a torrent app while torrents are off. */
const elsewhere = (stream: StremioStream, torrents: boolean) =>
  (stream.kind === 'torrent' && !torrents) || stream.kind === 'server';

const StreamLabel: FC<{ stream: StremioStream; fallback: string; note?: string | null }> = ({
  stream,
  fallback,
  note,
}) => {
  const { i18n } = useTranslation();
  const size = stream.size ? formatBytes(stream.size, i18n.language) : null;
  return (
    <span className="flex min-w-0 flex-1 flex-col gap-0.5">
      <span className="text-sm font-medium whitespace-pre-line">{stream.name ?? fallback}</span>
      {stream.description && (
        <span className="text-muted line-clamp-3 text-xs whitespace-pre-line">
          {stream.description}
        </span>
      )}
      {size && <span className="text-muted text-xs">{size}</span>}
      {note && <span className="text-muted text-xs">{note}</span>}
    </span>
  );
};

/** Streams only another app plays, folded under one line with how many (torrents first). */
const ElsewhereStreams: FC<{ streams: StremioStream[]; addonName: string }> = ({
  streams,
  addonName,
}) => {
  const { t } = useTranslation();
  const desktop = useIsDesktop();
  const open = useOpenStremioLink();
  const [expanded, setExpanded] = useState(false);
  const torrents = streams.filter((stream) => stream.kind === 'torrent').length;
  const summary =
    torrents === streams.length
      ? t('watch.torrentStreams', { count: torrents })
      : torrents === 0
        ? t('watch.serverStreams', { count: streams.length })
        : t('watch.otherAppStreams', { count: streams.length });
  return (
    <li className="flex flex-col gap-2">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setExpanded(!expanded)}
        className="bg-background text-muted flex items-center gap-2 rounded-xl px-4 py-3 text-left text-sm"
      >
        <Magnet aria-hidden size={16} className="shrink-0" />
        <span className="flex-1">{summary}</span>
        <ChevronDown
          aria-hidden
          size={16}
          className={`shrink-0 transition-transform ${expanded ? 'rotate-180' : ''}`}
        />
      </button>
      {expanded && (
        <ul className="flex flex-col gap-2">
          {streams.map((stream, index) => {
            const torrent = stream.kind === 'torrent';
            const items: OverflowItem[] = torrent
              ? [
                  {
                    label: t('watch.copyMagnet'),
                    icon: Copy,
                    onSelect: () => void navigator.clipboard?.writeText(stream.target),
                  },
                  ...(desktop
                    ? [
                        {
                          label: t('watch.openTorrentApp'),
                          icon: ExternalLink,
                          onSelect: () => open.mutate(stream.target),
                        },
                      ]
                    : []),
                ]
              : [];
            return (
              <li key={`${stream.target}-${index}`} className="flex items-stretch gap-2">
                <div className="bg-background flex min-w-0 flex-1 items-center gap-3 rounded-xl px-4 py-3 opacity-80">
                  <StreamLabel
                    stream={stream}
                    fallback={addonName}
                    note={t(torrent ? 'watch.torrentsOff' : 'watch.needsStremio')}
                  />
                </div>
                {items.length > 0 && (
                  <div className="bg-background grid shrink-0 place-items-center rounded-xl">
                    <OverflowMenu
                      label={t('watch.linkActions', { name: stream.name ?? addonName })}
                      items={items}
                    />
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {open.error && <SourceIssue compact>{open.error.message}</SourceIssue>}
    </li>
  );
};

/** One addon's streams: what plays or opens here, then what needs another app, folded. */
const StreamGroup: FC<{
  group: StremioStreamGroup;
  intent: PickIntent;
  busy: boolean;
  onPick: (stream: StremioStream, external?: boolean) => void;
  onSave: (stream: StremioStream) => void;
  /** Where a session would try it, and what played last time. */
  rank?: SourceRank | null;
  /** Torrents play here (Settings → Video player → Torrents). */
  torrents: boolean;
}> = ({ group, intent, busy, onPick, onSave, rank = null, torrents }) => {
  const { t } = useTranslation();
  const usable = (stream: StremioStream) =>
    intent === 'download' ? savable(stream, torrents) : usableHere(stream, torrents);
  const listed = group.streams.filter((stream) => !elsewhere(stream, torrents));
  // In the order a session tries them, the stream that played last time first.
  const here =
    rank && rank.links.length === listed.length
      ? rank.links.flatMap((index) => listed[index] ?? [])
      : listed;
  const worked = (stream: StremioStream) =>
    rank?.workedLink != null && listed[rank.workedLink] === stream;
  const other = group.streams.filter((stream) => elsewhere(stream, torrents));
  return (
    <section aria-label={group.addonName} className="flex flex-col gap-2">
      <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
        {group.addonName}
        {rank?.worked && !group.failing && (
          <span className="ml-2 font-normal tracking-normal normal-case">
            {t('videoPlayer.workedLastTime')}
          </span>
        )}
        {group.failing && (
          <span className="ml-2 font-normal tracking-normal normal-case">
            {t('watch.addonFailingLately')}
          </span>
        )}
      </h3>
      {group.error && (
        <SourceIssue compact source={group.addonName}>
          {group.error}
        </SourceIssue>
      )}
      <ul className="flex flex-col gap-2">
        {here.map((stream, index) => (
          <li key={`${stream.target}-${index}`} className="flex items-stretch gap-2">
            <button
              type="button"
              disabled={!usable(stream) || busy}
              onClick={() => onPick(stream)}
              className="bg-background flex min-w-0 flex-1 items-center gap-3 rounded-xl px-4 py-3 text-left disabled:opacity-60"
            >
              <StreamLabel
                stream={stream}
                fallback={group.addonName}
                note={
                  stream.kind === 'external'
                    ? t('watch.opensInBrowser')
                    : stream.kind === 'title'
                      ? t('watch.opensTitle')
                      : intent === 'download' && !savable(stream, torrents)
                        ? t('watch.cantDownload')
                        : stream.kind === 'torrent'
                          ? t('watch.torrent')
                          : null
                }
              />
              {worked(stream) && (
                <span className="text-muted flex shrink-0 items-center gap-1 text-xs">
                  <History aria-hidden size={14} />
                  {t('videoPlayer.workedLastTime')}
                </span>
              )}
              {usable(stream) &&
                (intent === 'download' && stream.kind !== 'title' ? (
                  <Download aria-hidden size={18} className="shrink-0" />
                ) : stream.kind === 'external' ? (
                  <ExternalLink aria-hidden size={18} className="shrink-0" />
                ) : stream.kind === 'title' ? (
                  <Clapperboard aria-hidden size={18} className="shrink-0" />
                ) : (
                  <Play aria-hidden size={18} className="shrink-0" />
                ))}
            </button>
            {intent === 'play' && savable(stream, torrents) && (
              <button
                type="button"
                aria-label={t('watch.downloadLink', {
                  name: stream.name ?? group.addonName,
                })}
                disabled={busy}
                onClick={() => onSave(stream)}
                className="bg-background text-muted grid w-12 shrink-0 place-items-center rounded-xl disabled:opacity-60"
              >
                <Download aria-hidden size={18} />
              </button>
            )}
            {intent === 'play' && stream.kind === 'direct' && (
              <div className="bg-background grid shrink-0 place-items-center rounded-xl">
                <OverflowMenu
                  label={t('watch.linkActions', { name: stream.name ?? group.addonName })}
                  items={[
                    {
                      label: t('watch.openElsewhere'),
                      icon: ExternalLink,
                      onSelect: () => onPick(stream, true),
                    },
                    {
                      label: t('watch.copyLink'),
                      icon: Copy,
                      onSelect: () => void navigator.clipboard?.writeText(stream.target),
                    },
                  ]}
                />
              </div>
            )}
          </li>
        ))}
        {intent === 'play' && other.length > 0 && (
          <ElsewhereStreams streams={other} addonName={group.addonName} />
        )}
      </ul>
    </section>
  );
};

/**
 * Streams for a movie or episode from every addon; picking one plays it (or opens the title or
 * page it links to), and direct ones download from their own button. Opened to download,
 * picking a stream saves it. Torrents and streams only Stremio plays are folded away with a count.
 */
export const StreamPicker: FC<{
  target: StreamTarget;
  intent?: PickIntent;
  onClose: () => void;
}> = ({ target, intent = 'play', onClose }) => {
  const { t } = useTranslation();
  useBackLayer(true, onClose);
  const navigate = useNavigate();
  const groups = useStremioStreams(target.kind, target.metaId, target.videoId);
  const torrents = useSettings().data?.torrents?.enabled ?? false;
  const play = useStremioPlay();
  const download = useDownloadStremio();
  const withStreams = (groups.data ?? []).filter(
    (group) => group.streams.length > 0 || group.error,
  );
  const { titleId = null, ...request } = target;
  const candidates = withStreams.map((group) => ({
    name: group.addonName,
    links: group.streams
      .filter((stream) => !elsewhere(stream, torrents))
      .map((stream) => ({
        label: streamLabel(stream, group.addonName),
        tag: stream.bingeGroup,
      })),
  }));
  // The order a session would try them in, what played last time first.
  const ranks = useSourceOrder(`stremio:${target.kind}:${target.metaId}`, target.title, candidates);
  const busy = play.isPending || download.isPending;
  const error = play.error ?? download.error;

  // A stream with subtitle tracks asks which to save first; the subtitle addons' follow the
  // languages picked.
  const [choosing, setChoosing] = useState<StremioStream | null>(null);
  const downloadStream = (stream: StremioStream, subtitles: string[] | null) =>
    download.mutate(
      { request: { ...request, stream }, titleId, subtitles },
      { onSuccess: onClose },
    );
  const save = (stream: StremioStream) =>
    stream.subtitles.length > 0 ? setChoosing(stream) : downloadStream(stream, null);
  const pick = (stream: StremioStream, external = false) => {
    if (stream.kind === 'title') {
      const split = stream.target.indexOf('/');
      onClose();
      void navigate({
        to: '/watch/title',
        search: { type: stream.target.slice(0, split), id: stream.target.slice(split + 1) },
      });
    } else if (intent === 'download') {
      save(stream);
    } else {
      play.mutate(
        { ...request, stream, external },
        { onSuccess: stream.kind === 'external' ? undefined : onClose },
      );
    }
  };
  return (
    <div className="motion-fade fixed inset-0 z-50 flex items-end justify-center bg-black/60 pt-[var(--safe-top)] pr-[var(--safe-right)] pl-[var(--safe-left)] sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="streams-title"
        className="bg-surface motion-rise flex max-h-[85vh] w-full max-w-md flex-col gap-3 rounded-t-2xl pt-5 pb-[calc(1rem+var(--safe-bottom))] sm:rounded-2xl"
      >
        <div className="flex items-start justify-between gap-3 px-5">
          <div className="min-w-0">
            <h2 id="streams-title" className="text-lg font-semibold">
              {t(intent === 'download' ? 'watch.downloadTitle' : 'watch.streams')}
            </h2>
            <p className="text-muted line-clamp-1 text-sm">
              {[target.title, target.subtitle].filter(Boolean).join(' · ')}
            </p>
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
          {choosing && (
            <SubtitleChoice
              source="stremio"
              tracks={choosing.subtitles}
              busy={download.isPending}
              onDownload={(urls) => downloadStream(choosing, urls)}
              onBack={() => setChoosing(null)}
            />
          )}
          {!choosing && groups.isPending && (
            <LoaderCircle
              role="status"
              aria-label={t('watch.findingStreams')}
              className="text-muted mx-auto animate-spin"
            />
          )}
          {groups.error && <SourceIssue>{groups.error.message}</SourceIssue>}
          {!groups.isPending && groups.data && withStreams.length === 0 && (
            <p className="text-muted text-sm">{t('watch.noStreams')}</p>
          )}
          {(choosing ? [] : inRankOrder(withStreams, ranks)).map(({ item: group, rank }) => (
            <StreamGroup
              key={`${group.addonId}-${rank?.index ?? group.addonName}`}
              group={group}
              rank={rank}
              intent={intent}
              busy={busy}
              onPick={pick}
              onSave={save}
              torrents={torrents}
            />
          ))}
          {error && <SourceIssue>{error.message}</SourceIssue>}
        </div>
      </div>
    </div>
  );
};
