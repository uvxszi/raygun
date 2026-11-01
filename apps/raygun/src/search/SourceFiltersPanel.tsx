import type { SourceFilter } from '@raygun/bindings';
import { Check, Minus, Search } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import type { FilterValues } from './useCatalogSearch';

type TagState = 'none' | 'include' | 'exclude';

const tagState = (values: FilterValues, key: string, tag: string): TagState => {
  const value = values[key];
  if (!value || typeof value !== 'object') return 'none';
  if (value.include.includes(tag)) return 'include';
  if (value.exclude.includes(tag)) return 'exclude';
  return 'none';
};

/** Tag chips cycle: not used → included → excluded → not used. */
const nextTagValues = (values: FilterValues, key: string, tag: string): FilterValues => {
  const current = values[key];
  const tags =
    current && typeof current === 'object'
      ? current
      : { include: [] as string[], exclude: [] as string[] };
  const state = tagState(values, key, tag);
  const include = tags.include.filter((known) => known !== tag);
  const exclude = tags.exclude.filter((known) => known !== tag);
  if (state === 'none') include.push(tag);
  if (state === 'include') exclude.push(tag);
  const next = { ...values };
  if (include.length === 0 && exclude.length === 0) delete next[key];
  else next[key] = { include, exclude };
  return next;
};

const TagsFilter: FC<{
  filter: Extract<SourceFilter, { kind: 'tags' }>;
  values: FilterValues;
  onChange: (values: FilterValues) => void;
}> = ({ filter, values, onChange }) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const shown = filter.options.filter((option) =>
    option.label.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  return (
    <fieldset className="flex flex-col gap-2">
      <legend className="text-muted mb-1 text-xs">
        {filter.label} · {t('search.filters.tagsHint')}
      </legend>
      {filter.options.length > 12 && (
        <label className="bg-background flex items-center gap-2 rounded-lg px-3 py-1.5">
          <Search aria-hidden size={14} className="text-muted" />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t('search.filters.findTag')}
            aria-label={t('search.filters.findTagIn', { label: filter.label })}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
          />
        </label>
      )}
      <div
        role="group"
        aria-label={filter.label}
        className="flex max-h-56 flex-wrap gap-2 overflow-y-auto"
      >
        {shown.map((option) => {
          const state = tagState(values, filter.key, option.value);
          return (
            <button
              key={option.value}
              type="button"
              aria-label={`${option.label}: ${t(`search.filters.tag.${state}`)}`}
              onClick={() => onChange(nextTagValues(values, filter.key, option.value))}
              className={`flex items-center gap-1 rounded-full px-3 py-1 text-sm ${
                state === 'include'
                  ? 'bg-primary/20 text-primary font-semibold'
                  : state === 'exclude'
                    ? 'bg-foreground text-background line-through'
                    : 'bg-background'
              }`}
            >
              {state === 'include' && <Check aria-hidden size={12} />}
              {state === 'exclude' && <Minus aria-hidden size={12} />}
              {option.label}
            </button>
          );
        })}
      </div>
    </fieldset>
  );
};

/** A source's own search filters (select, toggle and include/exclude tags). */
export const SourceFiltersPanel: FC<{
  filters: SourceFilter[];
  values: FilterValues;
  onChange: (values: FilterValues) => void;
}> = ({ filters, values, onChange }) => {
  const { t } = useTranslation();
  const set = (key: string, value: FilterValues[string] | undefined, fallback: unknown) => {
    const next = { ...values };
    if (value === undefined || value === fallback) delete next[key];
    else next[key] = value;
    onChange(next);
  };

  return (
    <section
      aria-label={t('search.filters.title')}
      className="bg-surface flex flex-col gap-4 rounded-xl px-4 py-3 text-sm"
    >
      {filters.map((filter) => {
        switch (filter.kind) {
          case 'select': {
            const fallback = filter.default ?? filter.options[0]?.value;
            const value = values[filter.key];
            return (
              <label key={filter.key} className="flex flex-col gap-1.5">
                <span className="text-muted text-xs">{filter.label}</span>
                <select
                  value={typeof value === 'string' ? value : (fallback ?? '')}
                  onChange={(event) => set(filter.key, event.target.value, fallback)}
                  className="bg-background rounded-lg px-3 py-2 outline-none"
                >
                  {filter.options.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
            );
          }
          case 'toggle': {
            const value = values[filter.key];
            const on = typeof value === 'boolean' ? value : filter.default;
            return (
              <label key={filter.key} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={on}
                  onChange={(event) => set(filter.key, event.target.checked, filter.default)}
                  className="accent-primary size-4"
                />
                {filter.label}
              </label>
            );
          }
          case 'tags':
            return (
              <TagsFilter key={filter.key} filter={filter} values={values} onChange={onChange} />
            );
        }
      })}
      {Object.keys(values).length > 0 && (
        <button type="button" onClick={() => onChange({})} className="text-primary w-fit">
          {t('search.filters.reset')}
        </button>
      )}
    </section>
  );
};
