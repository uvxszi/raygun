import type { SearchFilterPrefs, Settings } from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';
import { ListFilter } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';

import { PrefGroup, PrefSwitch } from '../settings/prefs';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { refreshSearches, searchFilterOf, storedFilters } from './searchFilter';
import type { SearchFilter } from './searchFilter';
import type { SearchType, SearchView } from './searchTypes';

/** A filter the sheet switches (the "No 18+" pill has its own place beside the kinds). */
type FilterField = Exclude<keyof SearchFilterPrefs, 'noAdult'>;

type FilterRow = { field: FilterField; label: string; hint: string };
type FilterGroup = { key: string; title?: string; rows: FilterRow[] };

/** Music's pills whose songs the song filters narrow; artists, albums and the rest carry none. */
const SONG_VIEWS: readonly SearchView[] = ['relevant', 'songs'];

/**
 * The filters a kind of search offers, as the settings say now: "18+ only" while 18+ content is
 * on (videos, comics, books), "Explicit only" while Explicit music is. All lists every kind's,
 * since its sections are those kinds' searches.
 */
const filterGroups = (
  type: SearchType,
  view: SearchView | undefined,
  settings: Settings,
  t: TFunction,
): FilterGroup[] => {
  const row = (field: FilterField, label: string, hint: string): FilterRow => ({
    field,
    label: t(`search.filter.${label}`),
    hint: t(`search.filter.${hint}`),
  });
  const adult = settings.showNsfw
    ? [row('adultOnly', 'adultOnly', type === 'all' ? 'adultOnlyAllHint' : 'adultOnlyHint')]
    : [];
  const songs = [
    ...(settings.explicitMusic ? [row('explicitOnly', 'explicitOnly', 'explicitOnlyHint')] : []),
    row('playableSongs', 'playable', 'playableSongsHint'),
  ];
  const videos = [row('playableVideos', 'playable', 'playableVideosHint')];
  const comics = [row('hideSavedComics', 'hideSaved', 'hideSavedHint')];
  const books = [row('hideSavedBooks', 'hideSaved', 'hideSavedHint')];
  switch (type) {
    case 'video':
      return [{ key: 'video', rows: [...adult, ...videos] }];
    case 'manga':
      return [{ key: 'manga', rows: [...adult, ...comics] }];
    case 'novel':
      return [{ key: 'novel', rows: [...adult, ...books] }];
    case 'music':
      return SONG_VIEWS.includes(view ?? 'relevant') ? [{ key: 'music', rows: songs }] : [];
    case 'all':
      return [
        ...(adult.length > 0 ? [{ key: 'adult', rows: adult }] : []),
        { key: 'music', title: t('search.music.songs'), rows: songs },
        { key: 'video', title: t('search.type.video'), rows: videos },
        { key: 'manga', title: t('search.type.manga'), rows: comics },
        { key: 'novel', title: t('search.type.novel'), rows: books },
      ];
    case 'library':
      return [];
  }
};

/** How many of the groups' filters are on, as they count now. */
const activeFilters = (groups: FilterGroup[], filter: SearchFilter) =>
  groups.flatMap((group) => group.rows).filter((row) => filter[row.field]).length;

/**
 * A kind of search's Filters (beside the "No 18+" pill): a sheet, or a popover beside the button
 * on wide windows, with a switch per filter and what each leaves out; the button counts those
 * on. The switches are kept in `Settings::searchFilter`, so they sync; "18+ only" and "No 18+"
 * turn each other off. Kinds without filters (Your library, Podcasts…) show no button.
 */
export const SearchFiltersButton: FC<{ type: SearchType; view?: SearchView }> = ({
  type,
  view,
}) => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  if (!settings) return null;
  const groups = filterGroups(type, view, settings, t);
  if (groups.length === 0) return null;
  const filter = searchFilterOf(settings);
  const count = activeFilters(groups, filter);

  const toggle = (field: FilterField) => {
    const on = !filter[field];
    const value: SearchFilterPrefs = {
      ...storedFilters(settings),
      [field]: on,
      // "18+ only" and "No 18+" turn each other off.
      ...(field === 'adultOnly' && on ? { noAdult: false } : {}),
    };
    update.mutate(
      { key: 'searchFilter', value },
      {
        // Rust asks other sources under "18+ only", and what a filter left out comes back
        // only when asked again; a filter turned on hides at once here.
        onSuccess: () => {
          if (!on || field === 'adultOnly') void refreshSearches(client);
        },
      },
    );
  };

  return (
    <>
      <button
        ref={setAnchor}
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        // Named apart from a source's own Filters, which can show below it.
        aria-label={
          count > 0 ? t('search.filter.buttonActive', { count }) : t('search.filter.title')
        }
        onClick={() => setOpen((shown) => !shown)}
        className={`flex shrink-0 items-center gap-1.5 rounded-full border px-3 py-1 text-xs ${
          count > 0 ? 'border-primary/40 bg-primary/15 text-primary' : 'border-border text-muted'
        }`}
      >
        <ListFilter aria-hidden size={14} />
        {t('search.filter.button')}
        {count > 0 && (
          <span
            aria-hidden
            data-testid="search-filter-count"
            className="bg-primary text-background grid h-4 min-w-4 place-items-center rounded-full px-1 text-3xs font-semibold"
          >
            {count}
          </span>
        )}
      </button>
      {open && (
        <Sheet title={t('search.filter.title')} anchor={anchor} onClose={() => setOpen(false)}>
          <div className="flex flex-col px-2">
            {groups.map((group) => (
              <PrefGroup key={group.key} title={group.title}>
                {group.rows.map((row) => (
                  <PrefSwitch
                    key={row.field}
                    title={row.label}
                    summary={row.hint}
                    checked={filter[row.field]}
                    onChange={() => toggle(row.field)}
                  />
                ))}
              </PrefGroup>
            ))}
          </div>
        </Sheet>
      )}
    </>
  );
};
