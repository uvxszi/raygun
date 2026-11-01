import type { StremioStream } from '@raygun/bindings';
import { LoaderCircle, Play, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useStremioPlay, useStremioStreams } from './useStremio';

/** What is being played, as the player shows it. */
export type StreamTarget = {
  kind: string;
  videoId: string;
  title: string;
  subtitle: string | null;
  poster: string | null;
};

const playable = (stream: StremioStream) => stream.kind === 'direct' || stream.kind === 'youTube';

/** Streams for a movie or episode from every addon; picking one plays it. */
export const StreamPicker: FC<{ target: StreamTarget; onClose: () => void }> = ({
  target,
  onClose,
}) => {
  const { t } = useTranslation();
  const groups = useStremioStreams(target.kind, target.videoId);
  const play = useStremioPlay();
  const withStreams = (groups.data ?? []).filter(
    (group) => group.streams.length > 0 || group.error,
  );

  const pick = (stream: StremioStream) =>
    play.mutate({ ...target, stream }, { onSuccess: onClose });

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
              {t('watch.streams')}
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
          {groups.error && (
            <p role="alert" className="text-primary text-sm wrap-anywhere">
              {groups.error.message}
            </p>
          )}
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
                <p className="text-muted text-sm wrap-anywhere">
                  {t('watch.streamsFailed', { message: group.error })}
                </p>
              )}
              <ul className="flex flex-col gap-2">
                {group.streams.map((stream, index) => (
                  <li key={`${stream.target}-${index}`}>
                    <button
                      type="button"
                      disabled={!playable(stream) || play.isPending}
                      onClick={() => pick(stream)}
                      className="bg-background flex w-full items-center gap-3 rounded-xl px-4 py-3 text-left disabled:opacity-60"
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
                      </span>
                      {playable(stream) && <Play aria-hidden size={18} className="shrink-0" />}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          ))}
          {play.error && (
            <p role="alert" className="text-primary text-sm wrap-anywhere">
              {play.error.message}
            </p>
          )}
        </div>
      </div>
    </div>
  );
};
