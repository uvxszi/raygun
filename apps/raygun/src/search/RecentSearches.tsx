import type { RecentSearch } from '@raygun/bindings';
import { History, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { kindOf, useForgetSearch, useRecentSearches } from './useRecentSearches';
import type { SearchType } from './searchTypes';

/** Kept searches shown before "Show all". */
const SHOWN = 8;

/**
 * Search before anything is typed: the searches made lately, newest first, each searched again
 * with a tap or forgotten with its ×. `type` puts that kind's searches first.
 */
export const RecentSearches: FC<{
  type: SearchType;
  onPick: (search: RecentSearch) => void;
}> = ({ type, onPick }) => {
  const { t } = useTranslation();
  const recent = useRecentSearches();
  const forget = useForgetSearch();
  const [all, setAll] = useState(false);
  const searches = recent.data ?? [];
  if (searches.length === 0) return null;
  // The kind being searched first, then the rest; each group newest first.
  const ordered =
    type === 'all'
      ? searches
      : [
          ...searches.filter((search) => kindOf(search) === type),
          ...searches.filter((search) => kindOf(search) !== type),
        ];
  const shown = all ? ordered : ordered.slice(0, SHOWN);
  return (
    <section
      aria-label={t('search.recent.title')}
      data-testid="recent-searches"
      className="flex flex-col"
    >
      <header className="flex items-center justify-between px-1">
        <h2 className="text-muted text-sm font-medium">{t('search.recent.title')}</h2>
        <button
          type="button"
          onClick={() => forget.mutate(null)}
          className="text-muted hover:text-foreground rounded-full px-2 py-1 text-sm"
        >
          {t('search.recent.clear')}
        </button>
      </header>
      <ul className="flex flex-col">
        {shown.map((search) => {
          const kind = kindOf(search);
          return (
            <li key={`${search.kind}:${search.query}`} className="flex items-center">
              <button
                type="button"
                onClick={() => onPick(search)}
                className="hover:bg-surface flex min-w-0 flex-1 items-center gap-3 rounded-xl px-1 py-2.5 text-left"
              >
                <History aria-hidden size={18} className="text-muted shrink-0" />
                <span className="truncate">{search.query}</span>
                {kind !== 'all' && (
                  <span className="text-muted shrink-0 text-xs">{t(`search.type.${kind}`)}</span>
                )}
              </button>
              <button
                type="button"
                aria-label={t('search.recent.remove', { query: search.query })}
                onClick={() => forget.mutate(search)}
                className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full"
              >
                <X aria-hidden size={16} />
              </button>
            </li>
          );
        })}
      </ul>
      {ordered.length > SHOWN && (
        <button
          type="button"
          aria-expanded={all}
          onClick={() => setAll(!all)}
          className="text-primary self-start rounded-full px-1 py-1.5 text-sm"
        >
          {all ? t('search.fewerSources') : t('search.recent.showAll', { count: ordered.length })}
        </button>
      )}
    </section>
  );
};
