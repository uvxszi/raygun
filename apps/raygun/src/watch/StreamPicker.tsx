import type { StremioStream } from '@raygun/bindings';
import { Copy, Download, ExternalLink, LoaderCircle, Play, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { OverflowMenu } from '../shell/OverflowMenu';
import type { PickIntent } from './ServerPicker';
import { useStremioPlay, useStremioStreams } from './useStremio';
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
  /** The movie or series, whose remembered speed, aspect and tracks the player uses. */
  metaId: string;
};

/** Only direct streams that aren't DASH can be saved. */
const savable = (stream: StremioStream) =>
  stream.kind === 'direct' && !stream.target.toLowerCase().includes('.mpd');

const playable = (stream: StremioStream) => stream.kind === 'direct' || stream.kind === 'youTube';

/**
 * Streams for a movie or episode from every addon; picking one plays it, and direct ones download
 * from their own button. Opened to download, picking a stream saves it.
 */
export const StreamPicker: FC<{
  target: StreamTarget;
  intent?: PickIntent;
  onClose: () => void;
}> = ({ target, intent = 'play', onClose }) => {
  const { t } = useTranslation();
  const groups = useStremioStreams(target.kind, target.videoId);
  const play = useStremioPlay();
  const download = useDownloadStremio();
  const withStreams = (groups.data ?? []).filter(
    (group) => group.streams.length > 0 || group.error,
  );
  const { titleId = null, ...request } = target;
  const busy = play.isPending || download.isPending;
  const error = play.error ?? download.error;

  const save = (stream: StremioStream) =>
    download.mutate({ request: { ...request, stream }, titleId }, { onSuccess: onClose });
  const pick = (stream: StremioStream, external = false) =>
    intent === 'download'
      ? save(stream)
      : play.mutate({ ...request, stream, external }, { onSuccess: onClose });
  const usable = (stream: StremioStream) =>
    intent === 'download' ? savable(stream) : playable(stream);

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-center bg-black/60 sm:items-center">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="streams-title"
        className="bg-surface flex max-h-[85vh] w-full max-w-md flex-col gap-3 rounded-t-2xl pt-5 pb-[calc(1rem+env(safe-area-inset-bottom))] sm:rounded-2xl"
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
          {groups.isPending && (
            <LoaderCircle
              role="status"
              aria-label={t('watch.findingStreams')}
              className="text-muted mx-auto animate-spin"
            />
          )}
          {groups.error && <SourceIssue>{groups.error.message}</SourceIssue>}
          {groups.data && withStreams.length === 0 && (
            <p className="text-muted text-sm">{t('watch.noStreams')}</p>
          )}
          {withStreams.map((group) => (
            <section
              key={group.addonId}
              aria-label={group.addonName}
              className="flex flex-col gap-2"
            >
              <h3 className="text-muted text-xs font-semibold tracking-wide uppercase">
                {group.addonName}
              </h3>
              {group.error && (
                <SourceIssue compact source={group.addonName}>
                  {group.error}
                </SourceIssue>
              )}
              <ul className="flex flex-col gap-2">
                {group.streams.map((stream, index) => (
                  <li key={`${stream.target}-${index}`} className="flex items-stretch gap-2">
                    <button
                      type="button"
                      disabled={!usable(stream) || busy}
                      onClick={() => pick(stream)}
                      className="bg-background flex min-w-0 flex-1 items-center gap-3 rounded-xl px-4 py-3 text-left disabled:opacity-60"
                    >
                      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                        <span className="text-sm font-medium whitespace-pre-line">
                          {stream.name ?? group.addonName}
                        </span>
                        {stream.description && (
                          <span className="text-muted line-clamp-3 text-xs whitespace-pre-line">
                            {stream.description}
                          </span>
                        )}
                        {!playable(stream) && (
                          <span className="text-muted text-xs">
                            {t(`watch.unsupported.${stream.kind}`)}
                          </span>
                        )}
                        {intent === 'download' && playable(stream) && !savable(stream) && (
                          <span className="text-muted text-xs">{t('watch.cantDownload')}</span>
                        )}
                      </span>
                      {usable(stream) &&
                        (intent === 'download' ? (
                          <Download aria-hidden size={18} className="shrink-0" />
                        ) : (
                          <Play aria-hidden size={18} className="shrink-0" />
                        ))}
                    </button>
                    {intent === 'play' && savable(stream) && (
                      <button
                        type="button"
                        aria-label={t('watch.downloadLink', {
                          name: stream.name ?? group.addonName,
                        })}
                        disabled={busy}
                        onClick={() => save(stream)}
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
                              onSelect: () => pick(stream, true),
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
              </ul>
            </section>
          ))}
          {error && <SourceIssue>{error.message}</SourceIssue>}
        </div>
      </div>
    </div>
  );
};
