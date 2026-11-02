import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioRow } from '../watch/StremioRow';
import { useStremioHome, useStremioSearch } from '../watch/useStremio';

/**
 * Movies and series from every Stremio addon that searches, a row per catalog; before anything
 * is typed, the addons' own catalogs (popular, top rated…) so Search doesn't start empty.
 */
export const VideoResults: FC<{ query: string; limit?: number }> = ({ query, limit }) => {
  const { t } = useTranslation();
  const blank = query.trim() === '';
  const found = useStremioSearch(query);
  const home = useStremioHome(blank);
  const search = blank ? home : found;
  if (search.error) {
    return (
      <p role="alert" className="text-primary text-sm wrap-anywhere">
        {search.error.message}
      </p>
    );
  }
  if (search.isPending) {
    return <p className="text-muted text-sm">{t('search.searching')}</p>;
  }
  if (search.data.length === 0) {
    // No addon has catalogs to show: nothing to say before a search.
    if (blank) return null;
    return <p className="text-muted text-sm">{t('search.noResults', { query })}</p>;
  }
  return (
    <div data-testid="video-results" className="flex flex-col gap-5">
      {search.data.slice(0, limit).map((row) => (
        <StremioRow key={`${row.addonId}-${row.kind}-${row.catalogId}`} row={row} browse={false} />
      ))}
    </div>
  );
};
