import type { ContinueWatching } from '@raygun/bindings';
import { Check, Clapperboard, Search, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { groupByDay, useDayLabel } from '../manga/groupByDay';
import { ConfirmButton } from '../shell/ConfirmButton';
import { EmptyState, Loading } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { useRemoveContinue, useResumeVideo, useWatchHistory } from '../watch/useWatch';
import { PageHeader } from './PageHeader';

/** The poster, or a plain frame when there's none or it can't load (offline). */
const Poster: FC<{ url: string | null }> = ({ url }) => {
  const [failed, setFailed] = useState(false);
  return url && !failed ? (
    <img
      src={url}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className="aspect-video w-20 shrink-0 rounded object-cover"
    />
  ) : (
    <span
      aria-hidden
      className="bg-background text-muted grid aspect-video w-20 shrink-0 place-items-center rounded"
    >
      <Clapperboard size={18} />
    </span>
  );
};

const HistoryRow: FC<{
  item: ContinueWatching;
  onResume: (key: string) => void;
  onRemove: (key: string) => void;
  busy: boolean;
}> = ({ item, onResume, onRemove, busy }) => {
  const { t } = useTranslation();
  const duration = item.durationMs ?? 0;
  const position = item.positionMs ?? 0;
  const name = [item.title, item.subtitle].filter(Boolean).join(' · ');
  return (
    <li aria-label={name} className="flex items-center gap-3 px-4 py-3">
      <button
        type="button"
        onClick={() => onResume(item.key)}
        className="flex min-w-0 flex-1 items-center gap-3 text-left"
      >
        <Poster url={item.poster} />
        <span className="flex min-w-0 flex-col gap-1">
          <span className="truncate font-medium">{item.title}</span>
          {item.subtitle && <span className="text-muted truncate text-xs">{item.subtitle}</span>}
          {item.watched ? (
            <span className="text-muted flex items-center gap-1 text-xs">
              <Check aria-hidden size={12} />
              {t('library.videos.watched')}
            </span>
          ) : (
            duration > 0 && (
              <span
                aria-hidden
                className="bg-background block h-1 w-32 overflow-hidden rounded-full"
              >
                <span
                  className="bg-primary block h-full origin-left"
                  style={{ transform: `scaleX(${Math.min(1, position / duration)})` }}
                />
              </span>
            )
          )}
        </span>
      </button>
      <ConfirmButton
        label={t('library.videos.remove', { title: name })}
        confirm={t('library.videos.removeConfirm')}
        icon={X}
        disabled={busy}
        onConfirm={() => onRemove(item.key)}
      />
    </li>
  );
};

/** Library → Videos → History: everything watched or started, newest first, by day (Aniyomi's). */
export const WatchHistoryPage: FC = () => {
  const { t } = useTranslation();
  const history = useWatchHistory();
  const resume = useResumeVideo();
  const remove = useRemoveContinue();
  const dayLabel = useDayLabel();

  return (
    <section data-testid="watch-history-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/library" title={t('library.videos.history')} />
      {history.error && (
        <SourceIssue compact title={t('library.videos.historyUnreadable')}>
          {history.error.message}
        </SourceIssue>
      )}
      {(resume.error ?? remove.error) && (
        <SourceIssue
          compact
          title={t(resume.error ? 'watch.playFailed' : 'library.videos.removeFailed')}
        >
          {(resume.error ?? remove.error)?.message}
        </SourceIssue>
      )}
      {history.isPending && <Loading label={t('manga.loading')} />}
      {history.data?.length === 0 && (
        <EmptyState
          icon={Clapperboard}
          title={t('library.videos.historyEmptyTitle')}
          action={{
            label: t('library.findInSearch'),
            icon: Search,
            to: '/browse',
            search: { tab: 'search', type: 'video' },
          }}
        />
      )}
      {history.data &&
        groupByDay(history.data, (item) => item.updatedAt ?? 0).map((group) => (
          <section key={group.day} className="flex flex-col gap-1">
            <h2 className="text-muted text-sm font-medium">{dayLabel(group.day)}</h2>
            <ul className="bg-surface flex flex-col rounded-xl">
              {group.items.map((item) => (
                <HistoryRow
                  key={item.key}
                  item={item}
                  busy={remove.isPending}
                  onResume={(key) => resume.mutate(key)}
                  onRemove={(key) => remove.mutate(key)}
                />
              ))}
            </ul>
          </section>
        ))}
    </section>
  );
};
