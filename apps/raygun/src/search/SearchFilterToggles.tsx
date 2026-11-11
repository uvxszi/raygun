import { useQueryClient } from '@tanstack/react-query';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { refreshSearches, searchFilterOf, storedFilters } from './searchFilter';
import { SearchFiltersButton } from './SearchFilters';
import type { SearchType, SearchView } from './searchTypes';

/**
 * Search's "No 18+" pill, kept in `Settings::searchFilter` so it syncs, then the kind's Filters
 * (`SearchFiltersButton`). The pill shows only while 18+ content is on in Settings (turning that
 * on switches the pill on again, in Rust); switched on, it turns "18+ only" off. Explicit songs
 * follow Settings → Explicit music.
 */
export const SearchFilterToggles: FC<{ type: SearchType; view?: SearchView }> = ({
  type,
  view,
}) => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const client = useQueryClient();
  if (!settings) return null;
  const filter = searchFilterOf(settings);
  const on = filter.pillShown && storedFilters(settings).noAdult;
  const toggle = () =>
    update.mutate(
      {
        key: 'searchFilter',
        value: { ...storedFilters(settings), noAdult: !on, ...(on ? {} : { adultOnly: false }) },
      },
      { onSuccess: () => void refreshSearches(client) },
    );
  return (
    // Kinds without filters, with 18+ content off, leave it empty: then it isn't there at all.
    <div
      role="group"
      aria-label={t('search.filter.label')}
      className="flex shrink-0 gap-2 empty:hidden"
    >
      {filter.pillShown && (
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
      )}
      <SearchFiltersButton type={type} view={view} />
    </div>
  );
};
