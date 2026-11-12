import type { SourceQueueItem, SourceQueueStatus } from '@raygun/bindings';
import type { TFunction } from 'i18next';
import {
  Ban,
  Check,
  CheckCheck,
  Clock,
  ListChecks,
  LoaderCircle,
  LogIn,
  Settings2,
  TriangleAlert,
  X,
} from 'lucide-react';
import { memo, useCallback, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ServerSignIn } from '../servers/ServerSheet';
import { SourceIssue } from '../shell/SourceIssue';
import { rowsEqual, useWindowedRows } from '../shell/windowedRows';
import type { RowProps } from '../shell/windowedRows';
import { useConfigureAddon } from '../watch/useStremio';
import { isBusy, useSourceQueue } from './sourceQueue';

/** The list scrolls in its own box, so a long one doesn't push the page down. */
const ownBox = (list: HTMLElement) => list.parentElement;

/** "9 added, 1 failed", for this list. */
const summaryOf = (t: TFunction, queue: SourceQueueStatus): string =>
  (
    [
      ['added', queue.added],
      ['alreadyAdded', queue.alreadyAdded],
      ['needsSignIn', queue.needsSignIn],
      ['needsSetup', queue.needsSetup],
      ['failed', queue.failed],
      ['cancelled', queue.cancelled],
    ] as const
  )
    .filter(([, count]) => count > 0)
    .map(([key, count]) => t(`sources.queue.count.${key}`, { count }))
    .join(', ');

/** "Raygun repository", "Stremio addon", "Jellyfin"… once it's known. */
const kindOf = (t: TFunction, item: SourceQueueItem): string | null => {
  switch (item.kind) {
    case 'repository':
      return t(`sources.repoKind.${item.format ?? 'raygun'}`);
    case 'server':
      return item.server ? t(`servers.kinds.${item.server}`) : null;
    case 'addon':
    case 'extension':
      return t(`sources.queue.kind.${item.kind}`);
    default:
      return null;
  }
};

const StateIcon: FC<{ item: SourceQueueItem }> = ({ item }) => {
  const props = { 'aria-hidden': true, size: 18, className: 'text-muted shrink-0' } as const;
  switch (item.state) {
    case 'waiting':
      return <Clock {...props} />;
    case 'checking':
    case 'adding':
      return <LoaderCircle {...props} className="text-primary shrink-0 animate-spin" />;
    case 'added':
      return <Check {...props} className="text-primary shrink-0" />;
    case 'alreadyAdded':
      return <CheckCheck {...props} />;
    case 'needsSignIn':
      return <LogIn {...props} className="text-primary shrink-0" />;
    case 'needsSetup':
      return <Settings2 {...props} className="text-primary shrink-0" />;
    case 'failed':
      return <TriangleAlert {...props} />;
    case 'cancelled':
      return <Ban {...props} />;
  }
};

type RowActions = {
  cancel: (id: number) => void;
  dismiss: (id: number) => void;
  signedIn: (id: number) => void;
  setUp: (url: string) => void;
  /** Opens or closes a server's sign-in under its row. */
  toggleSignIn: (id: number) => void;
};

/** One address: what it is once known, how it's going, and what can be done about it. */
const QueueRow = memo<{
  item: SourceQueueItem;
  signingIn: boolean;
  actions: RowActions;
  row: RowProps;
}>(({ item, signingIn, actions, row }) => {
  const { t } = useTranslation();
  const name = item.name ?? item.address;
  const kind = kindOf(t, item);
  const state = t(`sources.queue.state.${item.state}`);
  return (
    <li
      {...row}
      aria-label={name}
      data-state={item.state}
      className="border-background flex flex-col gap-1.5 border-b py-2 last:border-b-0"
    >
      <div className="flex items-center gap-3">
        <StateIcon item={item} />
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{name}</span>
          <span className="text-muted truncate text-xs">
            {[item.name ? item.address : null, kind, state].filter(Boolean).join(' · ')}
          </span>
        </div>
        {item.state === 'needsSignIn' && item.server && (
          <button
            type="button"
            aria-expanded={signingIn}
            onClick={() => actions.toggleSignIn(item.id)}
            className="bg-background shrink-0 rounded-full px-3 py-1.5 text-xs font-semibold"
          >
            {t('sources.queue.signIn')}
          </button>
        )}
        {item.state === 'needsSetup' && item.configureUrl && (
          <button
            type="button"
            onClick={() => item.configureUrl && actions.setUp(item.configureUrl)}
            className="bg-background shrink-0 rounded-full px-3 py-1.5 text-xs font-semibold"
          >
            {t('watch.setUp')}
          </button>
        )}
        {item.state !== 'adding' && (
          <button
            type="button"
            aria-label={t(isBusy(item.state) ? 'sources.queue.cancel' : 'sources.queue.dismiss', {
              name,
            })}
            onClick={() =>
              isBusy(item.state) ? actions.cancel(item.id) : actions.dismiss(item.id)
            }
            className="text-muted hover:text-foreground hover:bg-foreground/10 grid size-8 shrink-0 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        )}
      </div>
      {item.state === 'failed' && item.error && <SourceIssue compact>{item.error}</SourceIssue>}
      {signingIn && item.state === 'needsSignIn' && item.server && (
        <ServerSignIn
          kind={item.server}
          address={item.url ?? item.address}
          onConnected={() => actions.signedIn(item.id)}
        />
      )}
    </li>
  );
}, rowsEqual);
QueueRow.displayName = 'QueueRow';

/**
 * Sources → Add several's list as Rust works through it: where it is ("Adding sources · 3 of
 * 12", or held by Offline Mode), then its summary ("9 added, 1 failed"); every address with
 * how it's going, a sign-in for a server, a setup for an addon, the issue of one that failed.
 * Cancel all, Retry failed and Clear act on the whole list. Nothing shows without a list.
 */
export const SourceQueueCard: FC = () => {
  const { t } = useTranslation();
  const { queue, cancel, retry, dismiss, signedIn } = useSourceQueue();
  const configure = useConfigureAddon();
  const [signingIn, setSigningIn] = useState<number | null>(null);
  const items = queue.items;
  const keyOf = useCallback((index: number) => String(items[index]?.id ?? index), [items]);
  const { start, measure, ...rows } = useWindowedRows({
    count: items.length,
    keyOf,
    rowRem: 3.5,
    scrollerOf: ownBox,
    viewRem: 24,
  });
  const setUp = configure.mutate;
  const [actions] = useState<RowActions>(() => ({
    cancel: (id) => cancel(id),
    dismiss: (id) => dismiss(id),
    signedIn: (id) => {
      signedIn(id);
      setSigningIn(null);
    },
    setUp: (url) => setUp(url),
    toggleSignIn: (id) => setSigningIn((open) => (open === id ? null : id)),
  }));
  if (items.length === 0) return null;

  const batch = items.filter((item) => item.batch === queue.batch);
  const ended = batch.filter((item) => !isBusy(item.state)).length;
  const headline = queue.paused
    ? t('sources.queue.offline')
    : queue.loading
      ? t('sources.queue.loading')
      : queue.running
        ? t('sources.queue.progress', {
            position: Math.min(ended + 1, queue.total),
            total: queue.total,
          })
        : summaryOf(t, queue) || t('sources.queue.title');
  const cancellable = items.some((item) => item.state === 'waiting' || item.state === 'checking');
  const failed = items.some((item) => item.state === 'failed');
  const clearable = items.some((item) => !isBusy(item.state));
  const action = 'bg-background rounded-full px-3 py-1.5 text-xs font-semibold';

  return (
    <section
      aria-label={t('sources.queue.title')}
      data-testid="source-queue"
      className="bg-surface flex flex-col gap-2 rounded-xl px-4 py-3 text-sm"
    >
      <div className="flex items-center gap-3">
        {queue.running && !queue.paused ? (
          <LoaderCircle aria-hidden size={18} className="text-primary shrink-0 animate-spin" />
        ) : (
          <ListChecks aria-hidden size={18} className="text-muted shrink-0" />
        )}
        <div className="flex min-w-0 flex-1 flex-col">
          <p role="status" className="font-semibold">
            {headline}
          </p>
          {queue.running && queue.current && !queue.paused && (
            <p className="text-muted truncate text-xs">
              {t('sources.queue.now', { name: queue.current })}
            </p>
          )}
        </div>
      </div>
      {(cancellable || failed || clearable) && (
        <div className="flex flex-wrap justify-end gap-2">
          {cancellable && (
            <button type="button" onClick={() => cancel()} className={action}>
              {t('sources.queue.cancelAll')}
            </button>
          )}
          {failed && (
            <button type="button" onClick={retry} className={action}>
              {t('sources.queue.retryFailed')}
            </button>
          )}
          {clearable && (
            <button type="button" onClick={() => dismiss()} className={action}>
              {t('sources.queue.clear')}
            </button>
          )}
        </div>
      )}
      {configure.error && <SourceIssue compact>{configure.error.message}</SourceIssue>}
      <div className="-mx-1 max-h-96 overflow-y-auto px-1">
        <ul
          ref={start}
          aria-label={t('sources.queue.items')}
          className="flex flex-col"
          style={
            rows.windowed
              ? { paddingTop: rows.before, paddingBottom: rows.after, overflowAnchor: 'none' }
              : undefined
          }
        >
          {rows.slice(items).map(([item, index]) => (
            <QueueRow
              key={item.id}
              item={item}
              signingIn={signingIn === item.id}
              actions={actions}
              row={measure ? { ref: measure, 'data-index': index } : {}}
            />
          ))}
        </ul>
      </div>
    </section>
  );
};
