import { commands } from '@raygun/bindings';
import type { StorageArea, StorageSaving, StorageUsage } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import {
  CACHE_SIZES_MB,
  COVER_REFRESH_DAYS,
  COVER_SOURCE_SONGS,
  SONG_FACTS_DAYS,
  withCache,
} from './cacheDefaults';
import type { Cache } from './cacheDefaults';
import { LeftoverVideos } from './LeftoverVideos';
import { LyricsStorage } from './LyricsStorage';
import { PrefBlock, PrefChoice, PrefGroup, PrefInfo, PrefSwitch } from './prefs';
import { Problem } from './Problem';
import { useSettings, useUpdateSetting } from './useSettings';

const STORAGE = ['storage'] as const;
const SONG_MEMORY = ['songMemory'] as const;
const MB = 1024 * 1024;

/**
 * What's kept about songs, in the order it goes (Rust's `song_memory`): cover pictures, where
 * covers came from, everything else; with how much is kept and the two limits.
 */
const SongMemory: FC<{
  cache: Cache;
  busy: boolean;
  onChange: (patch: Partial<Cache>) => void;
}> = ({ cache, busy, onChange }) => {
  const { t, i18n } = useTranslation();
  const usage = useQuery({
    queryKey: SONG_MEMORY,
    queryFn: () => unwrapResult(commands.songMemoryUsage()),
  });
  const number = (value: number) => value.toLocaleString(i18n.language);
  const songs = (count: number) => t('songMemory.songs', { count, number: number(count) });
  const age = (days: number) =>
    days === 0 ? t('songMemory.never') : t('songMemory.days', { count: days });
  return (
    <PrefGroup title={t('songMemory.title')} summary={t('songMemory.summary')}>
      <PrefInfo title={t('songMemory.pictures')} summary={t('songMemory.picturesHint')} />
      <PrefInfo
        title={t('songMemory.sources')}
        summary={t('songMemory.sourcesHint', {
          kept: number(usage.data?.coverSources ?? 0),
          limit: songs(cache.coverSourceSongs),
        })}
      />
      <PrefChoice<number>
        title={t('songMemory.sourcesLimit')}
        value={cache.coverSourceSongs}
        disabled={busy}
        options={COVER_SOURCE_SONGS.map((value) => ({ value, label: songs(value) }))}
        onChange={(coverSourceSongs) => onChange({ coverSourceSongs })}
      />
      <PrefInfo
        title={t('songMemory.rest')}
        summary={t('songMemory.restHint', { songs: songs(usage.data?.songs ?? 0) })}
      />
      <PrefChoice<number>
        title={t('songMemory.restLimit')}
        value={cache.songFactsDays}
        disabled={busy}
        options={SONG_FACTS_DAYS.map((value) => ({ value, label: age(value) }))}
        onChange={(songFactsDays) => onChange({ songFactsDays })}
      />
      {usage.isError && (
        <PrefBlock>
          <Problem title={t('songMemory.usageFailed')} detail={usage.error.message} />
        </PrefBlock>
      )}
    </PrefGroup>
  );
};

const sizeLabel = (megabytes: number) => formatBytes(megabytes * MB);

/** What storing a cache's pictures as JPEG XL saves (covers, Rust's `cover_cache`). */
const ConvertedLine: FC<{ converted: StorageSaving }> = ({ converted }) => {
  const { t, i18n } = useTranslation();
  const original = converted.originalBytes ?? 0;
  const saved = Math.max(0, original - (converted.bytes ?? 0));
  const share = original > 0 ? saved / original : 0;
  return (
    <span className="text-muted text-xs">
      {t('storage.converted', {
        count: converted.files,
        number: converted.files.toLocaleString(i18n.language),
        saved: formatBytes(saved),
        percent: share.toLocaleString(i18n.language, {
          style: 'percent',
          maximumFractionDigits: 0,
        }),
      })}
    </span>
  );
};

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
          {usage.converted && usage.converted.files > 0 && (
            <ConvertedLine converted={usage.converted} />
          )}
        </span>
        {/* Downloads are removed where they're listed. */}
        {usage.area === 'downloads' && (
          <Link
            to="/downloads"
            className="text-primary shrink-0 rounded-full px-3 py-1.5 text-sm font-semibold"
          >
            {t('storage.manage')}
          </Link>
        )}
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
        <LeftoverVideos />
        {(usage.isError || clear.isError) && (
          <PrefBlock>
            <Problem
              title={t(clear.isError ? 'storage.clearFailed' : 'storage.usageFailed')}
              detail={(clear.error ?? usage.error)?.message}
            />
          </PrefBlock>
        )}
      </PrefGroup>
      {settings.data && (
        <PrefGroup title={t('storage.limits')}>
          <PrefChoice<number>
            title={t('storage.coverLimit')}
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
            checked={cache.clearPagesOnStart}
            disabled={busy}
            onChange={(clearPagesOnStart) => change({ clearPagesOnStart })}
          />
        </PrefGroup>
      )}
      {settings.data && <SongMemory cache={cache} busy={busy} onChange={change} />}
      <LyricsStorage />
    </>
  );
};
