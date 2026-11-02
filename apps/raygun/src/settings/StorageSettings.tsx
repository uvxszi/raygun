import { commands } from '@raygun/bindings';
import type { StorageArea, StorageUsage } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { CACHE_SIZES_MB, COVER_REFRESH_DAYS, withCache } from './cacheDefaults';
import type { Cache } from './cacheDefaults';
import { PrefBlock, PrefChoice, PrefGroup, PrefSwitch } from './prefs';
import { useSettings, useUpdateSetting } from './useSettings';

const STORAGE = ['storage'] as const;
const MB = 1024 * 1024;

const sizeLabel = (megabytes: number) => formatBytes(megabytes * MB);

const UsageRow: FC<{
  usage: StorageUsage;
  onClear: (area: StorageArea) => void;
  busy: boolean;
}> = ({ usage, onClear, busy }) => {
  const { t } = useTranslation();
  const name = t(`storage.areas.${usage.area}`);
  const bytes = usage.bytes ?? 0;
  const fraction = usage.limitBytes ? Math.min(1, bytes / usage.limitBytes) : null;
  return (
    <li data-testid="storage-row" aria-label={name} className="flex flex-col gap-1.5 px-1 py-3">
      <div className="flex items-center gap-3">
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="font-medium">{name}</span>
          <span className="text-muted text-sm">
            {usage.limitBytes
              ? t('storage.usedOf', {
                  used: formatBytes(bytes),
                  limit: formatBytes(usage.limitBytes),
                })
              : t('storage.used', { used: formatBytes(bytes), count: usage.files })}
          </span>
          <span className="text-muted text-xs">{t(`storage.hints.${usage.area}`)}</span>
        </span>
        {usage.clearable && (
          <button
            type="button"
            disabled={busy || bytes === 0}
            onClick={() => onClear(usage.area)}
            aria-label={t('storage.clearOne', { name })}
            className="text-primary shrink-0 rounded-full px-3 py-1.5 text-sm font-semibold disabled:opacity-40"
          >
            {t('storage.clear')}
          </button>
        )}
      </div>
      {fraction !== null && (
        <div
          role="progressbar"
          aria-label={t('storage.fullness', { name })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(fraction * 100)}
          className="bg-surface h-1.5 overflow-hidden rounded-full"
        >
          <div className="bg-primary h-full" style={{ width: `${fraction * 100}%` }} />
        </div>
      )}
    </li>
  );
};

/**
 * Settings → Data → Storage (M46.2): what each cache holds against its limit, with Clear, plus
 * the limits, how often covers are checked again, and emptying the page cache on start.
 */
export const StorageSettings: FC = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const settings = useSettings();
  const update = useUpdateSetting();
  const usage = useQuery({
    queryKey: STORAGE,
    queryFn: () => unwrapResult(commands.storageUsage()),
  });
  const clear = useMutation({
    mutationFn: (area: StorageArea) => unwrapResult(commands.storageClear(area)),
    onSuccess: (fresh) => queryClient.setQueryData(STORAGE, fresh),
  });
  const cache = withCache(settings.data?.cache);
  const change = (patch: Partial<Cache>) =>
    update.mutate(
      { key: 'cache', value: { ...cache, ...patch } },
      { onSuccess: () => void queryClient.invalidateQueries({ queryKey: STORAGE }) },
    );
  const busy = update.isPending || clear.isPending;
  const sizes = CACHE_SIZES_MB.map((value) => ({ value, label: sizeLabel(value) }));

  return (
    <>
      <PrefGroup title={t('storage.title')}>
        {usage.data?.map((row) => (
          <UsageRow key={row.area} usage={row} busy={busy} onClear={(area) => clear.mutate(area)} />
        ))}
        {(usage.isError || clear.isError) && (
          <PrefBlock>
            <p role="alert" className="text-primary text-sm">
              {(clear.error ?? usage.error)?.message}
            </p>
          </PrefBlock>
        )}
      </PrefGroup>
      {settings.data && (
        <PrefGroup title={t('storage.limits')}>
          <PrefChoice<number>
            title={t('storage.coverLimit')}
            summary={t('storage.coverLimitHint')}
            value={cache.coverCacheMb}
            disabled={busy}
            options={sizes}
            onChange={(coverCacheMb) => change({ coverCacheMb })}
          />
          <PrefChoice<number>
            title={t('storage.pageLimit')}
            value={cache.pageCacheMb}
            disabled={busy}
            options={sizes}
            onChange={(pageCacheMb) => change({ pageCacheMb })}
          />
          <PrefChoice<number>
            title={t('storage.refresh')}
            summary={t('storage.refreshHint')}
            value={cache.coverRefreshDays}
            disabled={busy}
            options={COVER_REFRESH_DAYS.map((value) => ({
              value,
              label:
                value === 0
                  ? t('storage.refreshNever')
                  : t('storage.refreshDays', { count: value }),
            }))}
            onChange={(coverRefreshDays) => change({ coverRefreshDays })}
          />
          <PrefSwitch
            title={t('storage.clearPagesOnStart')}
            summary={t('storage.clearPagesOnStartHint')}
            checked={cache.clearPagesOnStart}
            disabled={busy}
            onChange={(clearPagesOnStart) => change({ clearPagesOnStart })}
          />
        </PrefGroup>
      )}
    </>
  );
};
