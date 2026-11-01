import { ArrowDownWideNarrow, ArrowUpNarrowWide } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { DEFAULT_FILTER, LIBRARY_SORTS, isFiltering } from './libraryFilters';
import type { LibraryFilter, LibrarySort } from './libraryFilters';

const TOGGLES = ['unread', 'started', 'downloaded', 'completed'] as const;

const chip = (selected: boolean) =>
  `shrink-0 rounded-full px-3 py-1.5 text-sm ${selected ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;

/** Filters, sort and genre tags for the manga and novel libraries, in a sheet (Mihon's). */
export const LibraryFilterSheet: FC<{
  filter: LibraryFilter;
  tags: string[];
  onChange: (filter: LibraryFilter) => void;
  onClose: () => void;
}> = ({ filter, tags, onChange, onClose }) => {
  const { t } = useTranslation();
  const active = isFiltering(filter);

  const toggleTag = (tag: string) =>
    onChange({
      ...filter,
      tags: filter.tags.includes(tag)
        ? filter.tags.filter((known) => known !== tag)
        : [...filter.tags, tag],
    });

  return (
    <Sheet title={t('library.filter.panel')} onClose={onClose}>
      <section aria-label={t('library.filter.panel')} className="flex flex-col gap-4 text-sm">
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
              onChange={(event) => onChange({ ...filter, sort: event.target.value as LibrarySort })}
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
                  className={chip(filter.tags.includes(tag)).replace('bg-surface', 'bg-background')}
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
    </Sheet>
  );
};
