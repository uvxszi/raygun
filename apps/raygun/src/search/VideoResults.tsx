import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioRow } from '../watch/StremioRow';
import { useStremioSearch } from '../watch/useStremio';

/** Movies and series from every Stremio addon that searches, a row per catalog. */
export const VideoResults: FC<{ query: string; limit?: number }> = ({ query, limit }) => {
  const { t } = useTranslation();
  const search = useStremioSearch(query);
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
