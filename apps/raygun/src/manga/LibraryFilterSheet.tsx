import type { LibraryDisplayMode, LibraryTabs } from '@raygun/bindings';
import { ArrowDown, ArrowUp, Check, Minus, Shuffle } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { Sheet } from '../shell/Sheet';
import { DISPLAY_MODES, GRID_COLUMNS, LISTS, TAB_MODES, listKey } from './libraryDisplay';
import type { LibraryDisplay } from './libraryDisplay';
import {
  DEFAULT_FILTER,
  FLAG_FILTERS,
  LIBRARY_SORTS,
  isFiltering,
  nextTriState,
} from './libraryFilters';
import type { LibraryFilter, LibrarySort, TriState } from './libraryFilters';
import type { Medium } from './medium';

/** Words that read differently for videos ("Unwatched", "Last watched", "Latest episode"). */
const VIDEO_WORDS = new Set([
  'unread',
  'started',
  'lastRead',
  'latestChapter',
  'totalChapters',
  'fetchedAt',
]);

type Panel = 'filter' | 'sort' | 'display';

/** A chip that cycles off → only → without (Mihon's tri-state boxes). */
const TriChip: FC<{ label: string; state: TriState; onCycle: () => void }> = ({
  label,
  state,
  onCycle,
}) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onCycle}
      aria-label={`${label}: ${t(`library.filter.state.${state}`)}`}
      className={`flex shrink-0 items-center gap-1 rounded-full px-3 py-1.5 text-sm ${
        state === 'include'
          ? 'bg-primary/20 text-primary font-semibold'
          : state === 'exclude'
            ? 'bg-foreground/10 text-muted line-through'
            : 'bg-background'
      }`}
    >
      {state === 'include' && <Check aria-hidden size={14} />}
      {state === 'exclude' && <Minus aria-hidden size={14} />}
      {label}
    </button>
  );
};

const Section: FC<{ title: string; children: ReactNode }> = ({ title, children }) => (
  <div className="flex flex-col gap-1.5">
    <span className="text-muted text-xs">{title}</span>
    <div role="group" aria-label={title} className="flex flex-wrap gap-2">
      {children}
    </div>
  </div>
);

const Switch: FC<{ label: string; checked: boolean; onChange: (value: boolean) => void }> = ({
  label,
  checked,
  onChange,
}) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    onClick={() => onChange(!checked)}
    className="flex items-center justify-between gap-3 py-2 text-left"
  >
    <span>{label}</span>
    <span
      aria-hidden
      className={`relative h-6 w-10 shrink-0 rounded-full transition-colors ${checked ? 'bg-primary' : 'bg-muted/40'}`}
    >
      <span
        className={`bg-background absolute top-1 size-4 rounded-full transition-all ${checked ? 'left-5' : 'left-1'}`}
      />
    </span>
  </button>
);

/**
 * The library's settings sheet (Mihon's): Filter (tri-state flags, lists, genres, people), Sort
 * (tap the chosen sort again to flip it; Random reshuffles) and Display (layout, covers per row,
 * tabs, badges, continue button). Sort and layout follow the category when it keeps its own.
 */
export const LibraryFilterSheet: FC<{
  filter: LibraryFilter;
  tags: string[];
  people: string[];
  medium?: Medium;
  display: LibraryDisplay;
  /** The layout shown (the category's own, or the library's). */
  mode: LibraryDisplayMode;
  /** The sort shown (the category's own, or the library's). */
  sort: { sort: LibrarySort; descending: boolean };
  /** Whether sort and layout go to the open category. */
  perCategoryName?: string | null;
  onChange: (filter: LibraryFilter) => void;
  onSort: (sort: LibrarySort, descending: boolean, reshuffle: boolean) => void;
  onMode: (mode: LibraryDisplayMode) => void;
  onDisplay: (display: LibraryDisplay) => void;
  onClose: () => void;
}> = ({
  filter,
  tags,
  people,
  medium = 'manga',
  display,
  mode,
  sort,
  perCategoryName,
  onChange,
  onSort,
  onMode,
  onDisplay,
  onClose,
}) => {
  const { t } = useTranslation();
  const [panel, setPanel] = useState<Panel>('filter');
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const video = medium === 'video';
  const word = (group: 'filter' | 'sort', key: string) =>
    t(video && VIDEO_WORDS.has(key) ? `library.${group}.video.${key}` : `library.${group}.${key}`);
  const landscape = typeof window !== 'undefined' && window.innerWidth > window.innerHeight;
  const columns = landscape ? display.columnsLandscape : (settings?.gridColumns ?? 0);
  const setColumns = (value: number) =>
    landscape
      ? onDisplay({ ...display, columnsLandscape: value })
      : update.mutate({ key: 'gridColumns', value });

  const cycleIn = <Key extends string>(
    choices: Partial<Record<Key, TriState>>,
    key: Key,
  ): Partial<Record<Key, TriState>> => ({ ...choices, [key]: nextTriState(choices[key] ?? 'off') });

  return (
    <Sheet title={t('library.filter.panel')} onClose={onClose}>
      <div
        role="tablist"
        aria-label={t('library.filter.panel')}
        className="bg-background mb-3 flex rounded-full p-1 text-sm"
      >
        {(['filter', 'sort', 'display'] as const).map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={panel === id}
            onClick={() => setPanel(id)}
            className={`flex-1 rounded-full px-3 py-1.5 ${panel === id ? 'bg-primary/20 text-primary font-semibold' : ''}`}
          >
            {t(`library.filter.tabs.${id}`)}
          </button>
        ))}
      </div>

      {panel === 'filter' && (
        <section
          aria-label={t('library.filter.tabs.filter')}
          className="flex flex-col gap-4 text-sm"
        >
          <Section title={t('library.filter.show')}>
            {FLAG_FILTERS.map((flag) => (
              <TriChip
                key={flag}
                label={word('filter', flag)}
                state={filter.flags[flag] ?? 'off'}
                onCycle={() => onChange({ ...filter, flags: cycleIn(filter.flags, flag) })}
              />
            ))}
          </Section>
          <Section title={t('library.lists.title')}>
            {[...LISTS, 'none' as const].map((list) => (
              <TriChip
                key={list}
                label={list === 'none' ? t('library.lists.none') : t(listKey(list, video))}
                state={filter.lists[list] ?? 'off'}
                onCycle={() => onChange({ ...filter, lists: cycleIn(filter.lists, list) })}
              />
            ))}
          </Section>
          {tags.length > 0 && (
            <Section title={t('library.filter.tags')}>
              {tags.map((tag) => {
                const key = tag.toLocaleLowerCase();
                return (
                  <TriChip
                    key={key}
                    label={tag}
                    state={filter.tags[key] ?? 'off'}
                    onCycle={() =>
                      onChange({
                        ...filter,
                        tags: cycleIn(filter.tags, key) as Record<string, TriState>,
                      })
                    }
                  />
                );
              })}
            </Section>
          )}
          {people.length > 0 && (
            <Section title={t(video ? 'library.filter.cast' : 'library.filter.people')}>
              {people.map((person) => {
                const key = person.toLocaleLowerCase();
                return (
                  <TriChip
                    key={key}
                    label={person}
                    state={filter.people[key] ?? 'off'}
                    onCycle={() =>
                      onChange({
                        ...filter,
                        people: cycleIn(filter.people, key) as Record<string, TriState>,
                      })
                    }
                  />
                );
              })}
            </Section>
          )}
          {isFiltering(filter) && (
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...DEFAULT_FILTER,
                  query: filter.query,
                  sort: filter.sort,
                  descending: filter.descending,
                  seed: filter.seed,
                })
              }
              className="text-primary w-fit text-sm"
            >
              {t('library.filter.reset')}
            </button>
          )}
        </section>
      )}

      {panel === 'sort' && (
        <section aria-label={t('library.filter.tabs.sort')} className="flex flex-col text-sm">
          {perCategoryName && (
            <p className="text-muted pb-2 text-xs">
              {t('library.filter.forCategory', { name: perCategoryName })}
            </p>
          )}
          <div
            role="radiogroup"
            aria-label={t('library.filter.sortBy')}
            className="-mx-2 flex flex-col"
          >
            {LIBRARY_SORTS.map((key) => {
              const active = sort.sort === key;
              const Icon = key === 'random' ? Shuffle : sort.descending ? ArrowDown : ArrowUp;
              return (
                <button
                  key={key}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  onClick={() => {
                    if (key === 'random') onSort('random', false, true);
                    else onSort(key, active ? !sort.descending : sort.descending, false);
                  }}
                  className={`hover:bg-background flex items-center gap-3 rounded-xl px-3 py-2.5 text-left ${active ? 'text-primary font-semibold' : ''}`}
                >
                  <span className="grid size-5 shrink-0 place-items-center">
                    {active && <Icon aria-hidden size={16} />}
                  </span>
                  {word('sort', key)}
                  {active && key !== 'random' && (
                    <span className="sr-only">
                      {t(
                        sort.descending ? 'library.filter.descending' : 'library.filter.ascending',
                      )}
                    </span>
                  )}
                </button>
              );
            })}
          </div>
          <Switch
            label={t('library.display.pinsOnTop')}
            checked={display.pinsOnTop}
            onChange={(pinsOnTop) => onDisplay({ ...display, pinsOnTop })}
          />
        </section>
      )}

      {panel === 'display' && (
        <section
          aria-label={t('library.filter.tabs.display')}
          className="flex flex-col gap-4 text-sm"
        >
          {perCategoryName && (
            <p className="text-muted text-xs">
              {t('library.filter.forCategory', { name: perCategoryName })}
            </p>
          )}
          <Section title={t('library.display.mode')}>
            {DISPLAY_MODES.map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={mode === value}
                onClick={() => onMode(value)}
                className={`rounded-full px-3 py-1.5 ${mode === value ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`}
              >
                {t(`library.display.modes.${value}`)}
              </button>
            ))}
          </Section>
          {mode !== 'list' && (
            <Section
              title={t(
                landscape ? 'library.display.columnsLandscape' : 'library.display.columnsPortrait',
              )}
            >
              {GRID_COLUMNS.map((value) => (
                <button
                  key={value}
                  type="button"
                  aria-pressed={columns === value}
                  onClick={() => setColumns(value)}
                  className={`min-w-10 rounded-full px-3 py-1.5 ${columns === value ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`}
                >
                  {value === 0 ? t('settings.gridColumnsAuto') : value}
                </button>
              ))}
            </Section>
          )}
          <Section title={t('library.display.tabs')}>
            {TAB_MODES.map((value: LibraryTabs) => (
              <button
                key={value}
                type="button"
                aria-pressed={display.tabs === value}
                onClick={() => onDisplay({ ...display, tabs: value })}
                className={`rounded-full px-3 py-1.5 ${display.tabs === value ? 'bg-primary/20 text-primary font-semibold' : 'bg-background'}`}
              >
                {t(`library.display.tabModes.${value}`)}
              </button>
            ))}
          </Section>
          <div className="flex flex-col">
            <Switch
              label={t('library.display.showCounts')}
              checked={display.showCounts}
              onChange={(showCounts) => onDisplay({ ...display, showCounts })}
            />
            <Switch
              label={t(
                video ? 'library.display.continueWatching' : 'library.display.continueReading',
              )}
              checked={display.continueButton}
              onChange={(continueButton) => onDisplay({ ...display, continueButton })}
            />
            <Switch
              label={t('settings.unreadBadges')}
              checked={settings?.unreadBadges ?? true}
              onChange={(value) => update.mutate({ key: 'unreadBadges', value })}
            />
            <Switch
              label={t('settings.downloadBadges')}
              checked={settings?.downloadBadges ?? true}
              onChange={(value) => update.mutate({ key: 'downloadBadges', value })}
            />
            <Switch
              label={t('library.display.localBadge')}
              checked={display.localBadge}
              onChange={(localBadge) => onDisplay({ ...display, localBadge })}
            />
            <Switch
              label={t('library.display.perCategory')}
              checked={display.perCategory}
              onChange={(perCategory) => onDisplay({ ...display, perCategory })}
            />
          </div>
        </section>
      )}
    </Sheet>
  );
};
