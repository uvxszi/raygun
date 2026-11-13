import type { StremioCatalogRow, StremioItem } from '@raygun/bindings';
import { useMemo } from 'react';

import { useStremioSearchRows } from './useStremio';
import { videoTitleScore } from './videoMatch';

/** A match is at least this close to the title (search's own scale, 100 = the same). */
const CLOSE_ENOUGH = 60;
/** The most Stremio matches listed. */
const MOST = 12;

/** A movie or series in an addon's catalog, close to what's looked for. */
export type StremioMatch = { item: StremioItem; addonName: string; score: number };

/**
 * The close matches among addons' search rows: each title once (by its type and id, the first
 * addon naming it), without `exclude` (`<type>:<id>`, the title shown), closest first; other
 * seasons' entries ("Overlord II" for "Overlord") never are close.
 */
export const closeStremioMatches = (
  rows: readonly StremioCatalogRow[],
  query: string,
  exclude: string | null,
): StremioMatch[] => {
  const seen = new Set<string>();
  const matches: StremioMatch[] = [];
  for (const row of rows) {
    for (const item of row.items) {
      const key = `${item.kind}:${item.id}`;
      if (key === exclude || seen.has(key)) continue;
      seen.add(key);
      const score = videoTitleScore(item.name, query, true);
      if (score >= CLOSE_ENOUGH) matches.push({ item, addonName: row.addonName, score });
    }
  }
  return matches.sort((a, b) => b.score - a.score).slice(0, MOST);
};

/**
 * The same movie or series in the Stremio addons' catalogs, for Find in other sources on a
 * video's page: searched only when `enabled` (videos), the title shown left out.
 */
export const useStremioMatches = (query: string, exclude: string | null, enabled: boolean) => {
  const search = useStremioSearchRows(query, enabled && query !== '');
  const matches = useMemo(
    () => (enabled ? closeStremioMatches(search.rows, query, exclude) : []),
    [enabled, search.rows, query, exclude],
  );
  return { matches, pending: search.pending };
};
