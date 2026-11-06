import { commands } from '@raygun/bindings';
import type { LyricsCachePrefs, LyricsClearScope } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { PrefBlock, PrefChoice, PrefGroup, PrefSwitch } from './prefs';
import { Problem } from './Problem';
import { useSettings, useUpdateSetting } from './useSettings';

const USAGE = ['lyricsCache'] as const;
const DAYS = [0, 7, 14, 30, 60, 90, 180, 365];
const SCOPES: LyricsClearScope[] = ['all', 'notSaved', 'favorites', 'saved', 'downloaded'];

const DEFAULTS: Required<LyricsCachePrefs> = {
  forgetAfterDays: 30,
  keepFavorites: true,
  keepDownloaded: true,
  keepLocal: true,
};

/**
 * Settings → Data → Storage → Lyrics (M46.46): how many songs have lyrics and transcriptions kept,
 * clearing them for all songs or some, and how long lyrics of songs not played stay.
 */
export const LyricsStorage: FC = () => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const settings = useSettings();
  const update = useUpdateSetting();
  const [scope, setScope] = useState<LyricsClearScope>('all');
  const usage = useQuery({
    queryKey: USAGE,
    queryFn: () => unwrapResult(commands.lyricsCacheUsage()),
  });
  const clear = useMutation({
    mutationFn: (transcriptions: boolean) =>
      unwrapResult(commands.lyricsCacheClear(scope, transcriptions)),
    onSettled: () => {
      void client.invalidateQueries({ queryKey: USAGE });
      void client.invalidateQueries({ queryKey: ['lyrics'] });
    },
  });
  const saved = settings.data?.lyricsCache;
  const prefs: Required<LyricsCachePrefs> = {
    forgetAfterDays: saved?.forgetAfterDays ?? DEFAULTS.forgetAfterDays,
    keepFavorites: saved?.keepFavorites ?? DEFAULTS.keepFavorites,
    keepDownloaded: saved?.keepDownloaded ?? DEFAULTS.keepDownloaded,
    keepLocal: saved?.keepLocal ?? DEFAULTS.keepLocal,
  };
  const change = (patch: Partial<LyricsCachePrefs>) =>
    update.mutate({ key: 'lyricsCache', value: { ...prefs, ...patch } });

  return (
    <PrefGroup title={t('lyricsCache.title')}>
      <PrefBlock>
        <p data-testid="lyrics-cache-usage" className="text-sm">
          {usage.data
            ? t('lyricsCache.usage', {
                songs: t('lyricsCache.songs', { count: usage.data.songs }),
                transcriptions: t('lyricsCache.transcriptions', {
                  count: usage.data.transcriptions,
                }),
                translations: usage.data.translations ?? 0,
              })
            : ''}
        </p>
      </PrefBlock>
      <PrefChoice<LyricsClearScope>
        title={t('lyricsCache.clearFor')}
        value={scope}
        options={SCOPES.map((value) => ({ value, label: t(`lyricsCache.scope.${value}`) }))}
        onChange={setScope}
      />
      <PrefBlock>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={clear.isPending}
            onClick={() => clear.mutate(false)}
            className="bg-surface text-primary rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {t('lyricsCache.clearLyrics')}
          </button>
          <button
            type="button"
            disabled={clear.isPending}
            onClick={() => clear.mutate(true)}
            className="bg-surface text-primary rounded-full px-4 py-2 text-sm font-semibold disabled:opacity-50"
          >
            {t('lyricsCache.clearBoth')}
          </button>
        </div>
        {clear.isSuccess && (
          <p role="status" className="text-muted mt-2 text-xs">
            {t('lyricsCache.cleared', { count: clear.data })}
          </p>
        )}
        {clear.error && (
          <Problem
            title={t('lyricsCache.clearFailed')}
            detail={clear.error.message}
            className="mt-2"
          />
        )}
      </PrefBlock>
      {settings.data && (
        <>
          <PrefChoice<number>
            title={t('lyricsCache.forget')}
            value={prefs.forgetAfterDays}
            options={DAYS.map((value) => ({
              value,
              label: value === 0 ? t('lyricsCache.never') : t('lyricsCache.days', { count: value }),
            }))}
            onChange={(forgetAfterDays) => change({ forgetAfterDays })}
          />
          <PrefSwitch
            title={t('lyricsCache.keepFavorites')}
            checked={prefs.keepFavorites}
            onChange={(keepFavorites) => change({ keepFavorites })}
          />
          <PrefSwitch
            title={t('lyricsCache.keepDownloaded')}
            checked={prefs.keepDownloaded}
            onChange={(keepDownloaded) => change({ keepDownloaded })}
          />
          <PrefSwitch
            title={t('lyricsCache.keepLocal')}
            checked={prefs.keepLocal}
            onChange={(keepLocal) => change({ keepLocal })}
          />
        </>
      )}
    </PrefGroup>
  );
};
