import { commands } from '@raygun/bindings';
import type { DataCategory, DataUsage, DownloadsClear } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Sheet } from '../shell/Sheet';
import { ConfirmSheet } from './ConfirmSheet';
import { DATA_USAGE, countsLine, sortCategories } from './dataLabels';
import { PrefBlock, PrefGroup } from './prefs';
import { Problem } from './Problem';

/** Categories with a note of their own on what clearing does. */
const NOTES = new Set<DataCategory>([
  'comics',
  'books',
  'videos',
  'podcasts',
  'audiobooks',
  'settings',
  'pins',
  'sources',
  'caches',
  'aiModels',
]);

type Clearing = { category: DataCategory; downloads?: DownloadsClear };

const UsageRow: FC<{
  usage: DataUsage;
  busy: boolean;
  onClear: (category: DataCategory) => void;
}> = ({ usage, busy, onClear }) => {
  const { t } = useTranslation();
  const name = t(`data.categories.${usage.category}`);
  const line = countsLine(t, usage.counts, usage.bytes ?? 0);
  const empty = line === '';
  return (
    <li
      data-testid="data-row"
      aria-label={name}
      className="flex items-center gap-3 px-1 py-3"
      data-pref={name}
    >
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">{name}</span>
        <span className="text-muted text-sm">{empty ? t('data.usage.empty') : line}</span>
        <span className="text-muted text-xs">{t(`data.usage.${usage.scope}`)}</span>
      </span>
      <button
        type="button"
        disabled={busy || empty}
        onClick={() => onClear(usage.category)}
        aria-label={t('data.usage.clearOne', { name })}
        className="text-primary shrink-0 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-40"
      >
        {t('data.usage.clear')}
      </button>
    </li>
  );
};

/**
 * Settings → Data → On this device (M46.51): every kind of data with what it holds and its size,
 * and Clear for each. Clearing asks first and says where it reaches: synced kinds go on linked
 * devices too; the rest only here. Downloads ask whether the files go or only the entries waiting
 * or failed.
 */
export const DataCategoriesCard: FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [asking, setAsking] = useState<Clearing | null>(null);
  const [choosingDownloads, setChoosingDownloads] = useState(false);
  const usage = useQuery({
    queryKey: DATA_USAGE,
    queryFn: () => unwrapResult(commands.dataUsage()),
  });
  const clear = useMutation({
    mutationFn: ({ category, downloads }: Clearing) =>
      unwrapResult(commands.dataClear(category, downloads ?? null)),
    onSuccess: (left) => {
      queryClient.setQueryData(DATA_USAGE, left);
      // Lists elsewhere may show what just went.
      void queryClient.invalidateQueries({
        predicate: (query) => query.queryKey[0] !== DATA_USAGE[0],
      });
    },
  });
  const name = (category: DataCategory) => t(`data.categories.${category}`);
  const scopeOf = (category: DataCategory) =>
    usage.data?.find((row) => row.category === category)?.scope ?? 'device';

  return (
    <div data-testid="data-card">
      <PrefGroup title={t('data.usage.title')}>
        {usage.data &&
          sortCategories(usage.data).map((row) => (
            <UsageRow
              key={row.category}
              usage={row}
              busy={clear.isPending}
              onClear={(category) =>
                category === 'downloads' ? setChoosingDownloads(true) : setAsking({ category })
              }
            />
          ))}
        {(usage.isError || clear.isError) && (
          <PrefBlock>
            <Problem
              title={
                clear.isError && clear.variables
                  ? t('data.clear.failed', { name: name(clear.variables.category) })
                  : t('data.usage.failed')
              }
              detail={(clear.error ?? usage.error)?.message}
            />
          </PrefBlock>
        )}
      </PrefGroup>
      {choosingDownloads && (
        <Sheet title={t('data.clear.downloadsTitle')} onClose={() => setChoosingDownloads(false)}>
          <div data-testid="clear-downloads" className="flex flex-col gap-2 pb-1">
            {(['files', 'records'] as const).map((what) => (
              <button
                key={what}
                type="button"
                onClick={() => {
                  setChoosingDownloads(false);
                  setAsking({ category: 'downloads', downloads: what });
                }}
                className="bg-background hover:bg-surface flex flex-col rounded-xl px-3 py-2.5 text-left text-sm"
              >
                <span className="font-medium">{t(`data.clear.downloads.${what}`)}</span>
                <span className="text-muted text-xs">{t(`data.clear.downloadsHint.${what}`)}</span>
              </button>
            ))}
          </div>
        </Sheet>
      )}
      {asking && (
        <ConfirmSheet
          title={
            asking.downloads
              ? t(`data.clear.downloadsAsk.${asking.downloads}`)
              : t('data.clear.title', { name: name(asking.category) })
          }
          detail={[
            NOTES.has(asking.category) ? t(`data.clear.notes.${asking.category}`) : null,
            t(`data.clear.scope.${scopeOf(asking.category)}`),
          ]
            .filter(Boolean)
            .join(' ')}
          confirm={t('data.usage.clear')}
          onConfirm={() => clear.mutate(asking)}
          onClose={() => setAsking(null)}
        />
      )}
    </div>
  );
};
