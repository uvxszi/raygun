import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettingsSearch } from './useSettingsSearch';

/** What settings search found, each opening its screen at the row. */
export const SettingsSearchResults: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const results = useSettingsSearch(query);
  if (results.length === 0) {
    return (
      <p data-testid="settings-search-empty" className="text-muted px-1 py-6 text-center text-sm">
        {t('settings.searchNothing', { query: query.trim() })}
      </p>
    );
  }
  return (
    <ul aria-label={t('settings.searchResults')} className="flex flex-col">
      {results.map((result) => (
        <li key={result.key}>
          <Link
            to={result.to}
            hash={result.hash}
            className="hover:bg-surface flex items-center gap-4 rounded-xl px-1 py-3"
          >
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="truncate font-medium">{result.title}</span>
              <span className="text-muted truncate text-sm">{result.trail}</span>
            </span>
            <ChevronRight aria-hidden size={18} className="text-muted shrink-0" />
          </Link>
        </li>
      ))}
    </ul>
  );
};
