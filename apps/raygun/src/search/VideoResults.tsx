import { Link } from '@tanstack/react-router';
import { Compass } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioCatalogRowView, StremioRow } from '../watch/StremioRow';
import { useStremioCatalogs, useStremioSearchRows } from '../watch/useStremio';
import { SourceIssue } from '../shell/SourceIssue';

type Props = {
  query: string;
  limit?: number;
  /** Only the addons' rows of this kind (M46.25: the Movies and Series pills). */
  kind?: 'movie' | 'series';
};

/** Every addon's search results, a row per catalog, each shown as it arrives. */
const SearchRows: FC<Props> = ({ query, limit, kind }) => {
  const { t } = useTranslation();
  const search = useStremioSearchRows(query);
  const rows = kind ? search.rows.filter((row) => row.kind === kind) : search.rows;
  if (rows.length === 0) {
    return (
      <p className="text-muted text-sm">
        {search.pending ? t('search.searching') : t('search.noResults', { query })}
      </p>
    );
  }
  return (
    <div data-testid="video-results" className="flex flex-col gap-5">
      {rows.slice(0, limit).map((row) => (
        <StremioRow key={`${row.addonId}-${row.kind}-${row.catalogId}`} row={row} />
      ))}
      {search.pending && <p className="text-muted text-sm">{t('search.searching')}</p>}
    </div>
  );
};

/**
 * Movies and series from every Stremio addon: before anything is typed, the addons' own
 * catalogs as rows (Stremio's board: popular, top rated…, each loaded as it comes near), so
 * Search doesn't start empty; then every catalog that searches.
 */
export const VideoResults: FC<Props> = ({ query, limit, kind }) => {
  const { t } = useTranslation();
  const catalogs = useStremioCatalogs();
  if (query.trim() !== '') return <SearchRows query={query} limit={limit} kind={kind} />;
  if (catalogs.error) return <SourceIssue>{catalogs.error.message}</SourceIssue>;
  if (catalogs.isPending) return <p className="text-muted text-sm">{t('search.searching')}</p>;
  const board = catalogs.data.filter(
    (catalog) => catalog.board && (!kind || catalog.kind === kind),
  );
  // No addon has catalogs to show: nothing to say before a search.
  if (board.length === 0) return null;
  return (
    <div data-testid="video-results" className="flex flex-col gap-5">
      <Link
        to="/watch/catalog"
        search={kind ? { type: kind } : {}}
        className="bg-surface flex items-center gap-2 self-start rounded-full px-4 py-1.5 text-sm"
      >
        <Compass aria-hidden size={16} />
        {t('watch.discover')}
      </Link>
      {board.slice(0, limit).map((catalog) => (
        <StremioCatalogRowView
          key={`${catalog.addonId}-${catalog.kind}-${catalog.catalogId}`}
          catalog={catalog}
          quiet
        />
      ))}
    </div>
  );
};
