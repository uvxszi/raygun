import { useQueryClient } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { refreshSearches, searchFilterOf } from './searchFilter';

/**
 * Search's "No 18+" pill, kept in `Settings::searchFilter` so it syncs. It shows only while 18+
 * content is on in Settings (turning that on switches the pill on again, in Rust); explicit songs
 * follow Settings → Explicit music alone.
 */
export const SearchFilterToggles: FC = () => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const client = useQueryClient();
  if (!settings) return null;
  const filter = searchFilterOf(settings);
  if (!filter.pillShown) return null;
  const on = settings.searchFilter.noAdult;
  const toggle = () =>
    update.mutate(
      { key: 'searchFilter', value: { ...settings.searchFilter, noAdult: !on } },
      { onSuccess: () => void refreshSearches(client) },
    );
  return (
    <div role="group" aria-label={t('search.filter.label')} className="flex shrink-0 gap-2">
      <button
        type="button"
        role="switch"
        aria-checked={on}
        onClick={toggle}
        className={`flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-xs ${
          on ? 'border-primary/40 bg-primary/15 text-primary' : 'border-border text-muted'
        }`}
      >
        <span
          aria-hidden
          className={`size-2 rounded-full ${on ? 'bg-primary' : 'border-border border'}`}
        />
        {t('search.filter.noAdult')}
      </button>
    </div>
  );
};
