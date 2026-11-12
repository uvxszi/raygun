import type { StremioCatalogRow, StremioItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Compass } from 'lucide-react';
import { memo, useCallback, useDeferredValue, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioCatalogRowView, StremioRow } from '../watch/StremioRow';
import { useStremioAddons, useStremioCatalogs, useStremioSearchRows } from '../watch/useStremio';
import { SourceIssue } from '../shell/SourceIssue';
import { useGrouped } from './arrangement';
import { ArrangeToggle } from './ArrangeToggle';
import { keepAddon, keepStremioItems, useSearchFilter } from './searchFilter';
import { kindKey, mergeKinds } from './videoKinds';
import { ROW_TITLES, VideoKindSection } from './VideoKindSection';

/** The kinds Videos has a pill of its own for. */
export type VideoKind = 'movie' | 'series';

type Props = {
  query: string;
  /** A preview (Search → All): at most this many sections (rows By source), without choices. */
  limit?: number;
  /** Only the addons' titles of this kind (M46.25: the Movies and Series pills). */
  kind?: VideoKind;
  /** See all of Movies or Series opens its pill (Relevant). */
  onKind?: (kind: VideoKind) => void;
};

const NO_ADULT: ReadonlySet<string> = new Set();

const isVideoKind = (kind: string): kind is VideoKind => kind === 'movie' || kind === 'series';

/**
 * Whether an addon's rows show: an addon rated for adults' don't while 18+ is left out (Search's
 * "No 18+"), the others' don't under "18+ only"; Rust doesn't ask them then, this covers rows
 * kept from before. Its titles follow Videos' "Playable only" (`keep`). The same functions while
 * the filter and the addons are the same.
 */
const useAddonFilter = () => {
  const filter = useSearchFilter();
  const addons = useStremioAddons().data;
  return useMemo(() => {
    const adult: ReadonlySet<string> = addons
      ? new Set(addons.filter((addon) => addon.adult).map((addon) => addon.id))
      : NO_ADULT;
    return {
      adult,
      shown: (addonId: string) => keepAddon(filter, adult.has(addonId)),
      keep: (items: StremioItem[]) => keepStremioItems(filter, items),
    };
  }, [filter, addons]);
};

/**
 * What the addons found, Best matches (the default): a section per kind (Movies, Series,
 * Channels…) holding every addon's titles of it, ranked together, a title several addons have
 * shown once with how many have it; on a kind's own pill its section is a grid of every title.
 * By source: a row per addon catalog, as each answered. Drawn again only when what it shows
 * changes, not as the switch above it answers a tap.
 */
const AddonResultsView: FC<{
  rows: StremioCatalogRow[];
  bySource: boolean;
  /** The search, trimmed. */
  query: string;
  limit: number | undefined;
  /** Its kind's own pill: every title in a grid. */
  grid: boolean;
  /** Addons rated for adults: their titles are marked 18+. */
  adult: ReadonlySet<string>;
  onKind?: (kind: VideoKind) => void;
}> = ({ rows, bySource, query, limit, grid, adult, onKind }) => {
  const sections = useMemo(
    () => (bySource ? [] : mergeKinds(rows, query)),
    [bySource, rows, query],
  );
  // Kinds whose See all opened them here, for this search.
  const [opened, setOpened] = useState({ query: '', kinds: [] as string[] });
  const openedKinds = opened.query === query ? opened.kinds : [];
  const onMore = useCallback(
    (key: string) => {
      if (onKind && isVideoKind(key)) onKind(key);
      else
        setOpened((current) => ({
          query,
          kinds: [...(current.query === query ? current.kinds : []), key],
        }));
    },
    [onKind, query],
  );
  if (bySource) {
    return rows
      .slice(0, limit)
      .map((row) => <StremioRow key={`${row.addonId}-${row.kind}-${row.catalogId}`} row={row} />);
  }
  return sections.slice(0, limit).map((section) => {
    const whole = grid || openedKinds.includes(section.key);
    return (
      <VideoKindSection
        // A new search starts each section again (its first covers, its first page).
        key={`${query}\n${section.key}`}
        section={section}
        layout={whole ? 'grid' : 'row'}
        adult={adult}
        // A preview (Search → All) leads to Videos as a whole.
        more={!whole && limit === undefined && section.titles.length > ROW_TITLES}
        onMore={onMore}
      />
    );
  });
};

const AddonResults = memo(AddonResultsView);

/**
 * Every addon's search results as they arrive, Best matches or By source (`AddonResults`). With
 * more than one addon searching, the switch between them shows above them, except in a preview.
 */
const SearchRows: FC<Props> = ({ query, limit, kind, onKind }) => {
  const { t } = useTranslation();
  const search = useStremioSearchRows(query);
  const catalogs = useStremioCatalogs();
  const { adult, shown, keep } = useAddonFilter();
  const [grouped, setGrouped] = useGrouped('video');
  // The switch answers at once; the results follow, drawn without holding the tap up.
  const bySource = useDeferredValue(grouped);
  const rows = useMemo(
    () =>
      search.rows
        .filter((row) => (!kind || kindKey(row.kind) === kind) && shown(row.addonId))
        .map((row): StremioCatalogRow => {
          const items = keep(row.items);
          return items === row.items ? row : { ...row, items };
        })
        .filter((row) => row.items.length > 0),
    [search.rows, kind, shown, keep],
  );
  // Addons that search for what's shown: with more than one, how they're arranged is a choice.
  const addons = useMemo(
    () =>
      new Set(
        (catalogs.data ?? [])
          .filter(
            (catalog) =>
              catalog.searchable &&
              (!kind || kindKey(catalog.kind) === kind) &&
              shown(catalog.addonId),
          )
          .map((catalog) => catalog.addonId),
      ).size,
    [catalogs.data, kind, shown],
  );
  if (rows.length === 0) {
    return (
      <p className="text-muted text-sm">
        {search.pending ? t('search.searching') : t('search.noResults', { query })}
      </p>
    );
  }
  return (
    <div data-testid="video-results" className="flex flex-col gap-5">
      {limit === undefined && addons > 1 && (
        <ArrangeToggle grouped={grouped} onChange={setGrouped} className="self-end" />
      )}
      <AddonResults
        rows={rows}
        bySource={bySource}
        query={query.trim()}
        limit={limit}
        grid={Boolean(kind)}
        adult={adult}
        onKind={onKind}
      />
      {search.pending && <p className="text-muted text-sm">{t('search.searching')}</p>}
    </div>
  );
};

/**
 * Movies and series from every Stremio addon: before anything is typed, the addons' own
 * catalogs as rows (Stremio's board: popular, top rated…, each loaded as it comes near), so
 * Search doesn't start empty; then every catalog that searches, a section per kind.
 */
export const VideoResults: FC<Props> = ({ query, limit, kind, onKind }) => {
  const { t } = useTranslation();
  const catalogs = useStremioCatalogs();
  const { shown, keep } = useAddonFilter();
  if (query.trim() !== '') {
    return <SearchRows query={query} limit={limit} kind={kind} onKind={onKind} />;
  }
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
          keep={keep}
        />
      ))}
    </div>
  );
};
