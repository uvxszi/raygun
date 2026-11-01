import { ArrowDownWideNarrow, ArrowUpNarrowWide, Search, SlidersHorizontal } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { DEFAULT_FILTER, LIBRARY_SORTS, isFiltering } from './libraryFilters';
import type { LibraryFilter, LibrarySort } from './libraryFilters';

const TOGGLES = ['unread', 'started', 'downloaded', 'completed'] as const;

const chip = (selected: boolean) =>
  `shrink-0 rounded-full px-3 py-1.5 text-sm ${selected ? 'bg-primary text-background font-semibold' : 'bg-surface'}`;

/** Search, filters, sort and genre tags for the manga and novel libraries. */
export const LibraryControls: FC<{
  filter: LibraryFilter;
  tags: string[];
  onChange: (filter: LibraryFilter) => void;
}> = ({ filter, tags, onChange }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const active = isFiltering(filter);

  const toggleTag = (tag: string) =>
    onChange({
      ...filter,
      tags: filter.tags.includes(tag)
        ? filter.tags.filter((known) => known !== tag)
        : [...filter.tags, tag],
    });

  return (
    <div className="flex flex-col gap-2">
      <div className="flex gap-2">
        <label className="bg-surface flex min-w-0 flex-1 items-center gap-2 rounded-lg px-3 py-2">
          <Search aria-hidden size={16} className="text-muted" />
          <input
            type="search"
            value={filter.query}
            onChange={(event) => onChange({ ...filter, query: event.target.value })}
            placeholder={t('library.filter.searchPlaceholder')}
            aria-label={t('library.filter.search')}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </label>
        <button
          type="button"
          aria-expanded={open}
          aria-label={t('library.filter.open')}
          onClick={() => setOpen(!open)}
          className={`relative grid size-10 shrink-0 place-items-center rounded-lg ${open ? 'bg-primary text-background' : 'bg-surface'}`}
        >
          <SlidersHorizontal aria-hidden size={18} />
          {active && !open && (
            <span
              aria-hidden
              className="bg-primary absolute top-1.5 right-1.5 size-2 rounded-full"
            />
          )}
        </button>
      </div>
      {open && (
        <section
          aria-label={t('library.filter.panel')}
          className="bg-surface flex flex-col gap-3 rounded-xl px-4 py-3 text-sm"
        >
          <div className="flex flex-col gap-1.5">
            <span className="text-muted text-xs">{t('library.filter.show')}</span>
            <div className="flex flex-wrap gap-2">
              {TOGGLES.map((key) => (
                <button
                  key={key}
                  type="button"
                  aria-pressed={filter[key]}
                  onClick={() => onChange({ ...filter, [key]: !filter[key] })}
                  className={chip(filter[key]).replace('bg-surface', 'bg-background')}
                >
                  {t(`library.filter.${key}`)}
                </button>
              ))}
            </div>
          </div>
          <div className="flex items-end gap-2">
            <label className="flex min-w-0 flex-1 flex-col gap-1.5">
              <span className="text-muted text-xs">{t('library.filter.sortBy')}</span>
              <select
                value={filter.sort}
                onChange={(event) =>
                  onChange({ ...filter, sort: event.target.value as LibrarySort })
                }
                className="bg-background rounded-lg px-3 py-2 outline-none"
              >
                {LIBRARY_SORTS.map((sort) => (
                  <option key={sort} value={sort}>
                    {t(`library.sort.${sort}`)}
                  </option>
                ))}
              </select>
            </label>
            <button
              type="button"
              aria-label={t(
                filter.descending ? 'library.filter.descending' : 'library.filter.ascending',
              )}
              aria-pressed={filter.descending}
              onClick={() => onChange({ ...filter, descending: !filter.descending })}
              className="bg-background grid size-10 place-items-center rounded-lg"
            >
              {filter.descending ? (
                <ArrowDownWideNarrow aria-hidden size={18} />
              ) : (
                <ArrowUpNarrowWide aria-hidden size={18} />
              )}
            </button>
          </div>
          {tags.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-muted text-xs">{t('library.filter.tags')}</span>
              <div
                role="group"
                aria-label={t('library.filter.tags')}
                className="flex flex-wrap gap-2"
              >
                {tags.map((tag) => (
                  <button
                    key={tag}
                    type="button"
                    aria-pressed={filter.tags.includes(tag)}
                    onClick={() => toggleTag(tag)}
                    className={chip(filter.tags.includes(tag)).replace(
                      'bg-surface',
                      'bg-background',
                    )}
                  >
                    {tag}
                  </button>
                ))}
              </div>
            </div>
          )}
          {active && (
            <button
              type="button"
              onClick={() =>
                onChange({
                  ...DEFAULT_FILTER,
                  query: filter.query,
                  sort: filter.sort,
                  descending: filter.descending,
                })
              }
              className="text-primary w-fit text-sm"
            >
              {t('library.filter.reset')}
            </button>
          )}
        </section>
      )}
    </div>
  );
};
