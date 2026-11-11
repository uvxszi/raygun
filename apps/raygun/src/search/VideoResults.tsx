import { Link } from '@tanstack/react-router';
import { Compass } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioCatalogRowView, StremioRow } from '../watch/StremioRow';
import { useStremioAddons, useStremioCatalogs, useStremioSearchRows } from '../watch/useStremio';
import { SourceIssue } from '../shell/SourceIssue';
import { useSearchFilter } from './searchFilter';

type Props = {
  query: string;
  limit?: number;
  /** Only the addons' rows of this kind (M46.25: the Movies and Series pills). */
  kind?: 'movie' | 'series';
};

/**
 * Whether an addon's rows show: an addon rated for adults' don't while 18+ is left out (Search's
 * "No 18+"); Rust doesn't ask it then, this covers rows kept from before.
 */
const useAddonShown = () => {
  const { noAdult } = useSearchFilter();
  const addons = useStremioAddons();
  const adult = new Set(
    (addons.data ?? []).filter((addon) => addon.adult).map((addon) => addon.id),
  );
  return (addonId: string) => !(noAdult && adult.has(addonId));
};

/** Every addon's search results, a row per catalog, each shown as it arrives. */
const SearchRows: FC<Props> = ({ query, limit, kind }) => {
  const { t } = useTranslation();
  const search = useStremioSearchRows(query);
  const shown = useAddonShown();
  const rows = search.rows.filter((row) => (!kind || row.kind === kind) && shown(row.addonId));
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
  const shown = useAddonShown();
  if (query.trim() !== '') return <SearchRows query={query} limit={limit} kind={kind} />;
  if (catalogs.error) return <SourceIssue>{catalogs.error.message}</SourceIssue>;
  if (catalogs.isPending) return <p className="text-muted text-sm">{t('search.searching')}</p>;
  const board = catalogs.data.filter(
    (catalog) => catalog.board && (!kind || catalog.kind === kind) && shown(catalog.addonId),
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
