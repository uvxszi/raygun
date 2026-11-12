import type { StremioCatalogRow, StremioItem } from '@raygun/bindings';

import { parseTitle, seasonKey, videoTitleScore } from '../watch/videoMatch';
import { rankLists } from './rankResults';

/** One addon's copy of a title. */
export type TitleCopy = { item: StremioItem; addonId: string; addonName: string };

/**
 * A title as Videos' search shows it: what the addons that have it said, the best ranked copy
 * first (its card opens that one), then the same title from the other addons, one copy each.
 */
export type MergedTitle = {
  /** The best copy's id: unique within its kind. */
  key: string;
  /** What its card shows: the best copy, with a poster, year or rating from another when it
   * has none. */
  item: StremioItem;
  copies: TitleCopy[];
};

/** Every addon's titles of one kind (Movies, Series, Channels…), ranked as one list. */
export type KindSection = {
  /** The kind as compared: lower case (`movie`, `series`, `anime`…). */
  key: string;
  /** The kind as the first addon wrote it. */
  kind: string;
  titles: MergedTitle[];
};

/** The kind a row lists, as compared: `Anime` and `anime` are one kind. */
export const kindKey = (kind: string) => kind.trim().toLowerCase();

/** The first year a release info names ("2019", "2008–2013", "2019-"). */
const releaseYear = (releaseInfo: string | null) => {
  const found = /\d{4}/.exec(releaseInfo ?? '')?.[0];
  return found ? Number(found) : null;
};

/** Both years unknown, or within a year of each other (as `sameSeason` allows). */
const yearsAgree = (first: number | null, second: number | null) =>
  first === null || second === null ? first === second : Math.abs(first - second) <= 1;

const nameOf = (item: StremioItem) => item.name || item.id;

/** The best copy (the first) as its card shows it, a gap filled from the copies after it. */
const shownOf = (copies: readonly TitleCopy[]): StremioItem => {
  const best = copies[0]!.item;
  const others = copies.slice(1).map((copy) => copy.item);
  if (others.length === 0 || (best.poster && best.releaseInfo && best.rating)) return best;
  const pictured = best.poster ? best : others.find((item) => item.poster);
  return {
    ...best,
    poster: pictured?.poster ?? null,
    posterShape: pictured?.posterShape ?? best.posterShape,
    releaseInfo: best.releaseInfo ?? others.find((item) => item.releaseInfo)?.releaseInfo ?? null,
    rating: best.rating ?? others.find((item) => item.rating)?.rating ?? null,
  };
};

type Merging = { key: string; copies: TitleCopy[]; year: number | null };

/**
 * The ranked copies as titles: a copy joins the title of a copy ranked before it with the same id
 * (addons that use IMDb ids agree on them), or else with the same name, season and year (an
 * addon with ids of its own: Kitsu's, TMDB's). An addon never meets itself: its own copies of
 * a title count once, and two of its titles with the same name stay two.
 */
const mergeCopies = (copies: readonly TitleCopy[]): MergedTitle[] => {
  const titles: Merging[] = [];
  const byId = new Map<string, Merging>();
  const byName = new Map<string, Merging[]>();
  for (const copy of copies) {
    const sameId = byId.get(copy.item.id);
    if (sameId) {
      if (!sameId.copies.some((have) => have.addonId === copy.addonId)) sameId.copies.push(copy);
      continue;
    }
    const info = parseTitle(nameOf(copy.item));
    const name = seasonKey(info);
    const year = info.year ?? releaseYear(copy.item.releaseInfo);
    const named = name ? (byName.get(name) ?? []) : [];
    const same = named.find(
      (title) =>
        yearsAgree(title.year, year) && !title.copies.some((have) => have.addonId === copy.addonId),
    );
    if (same) {
      same.copies.push(copy);
      byId.set(copy.item.id, same);
      continue;
    }
    const title: Merging = { key: copy.item.id, copies: [copy], year };
    titles.push(title);
    byId.set(copy.item.id, title);
    if (name) byName.set(name, [...named, title]);
  }
  return titles.map(({ key, copies: found }) => ({ key, item: shownOf(found), copies: found }));
};

/**
 * The addons' search rows as one section per kind, in the order the kinds first come (the
 * catalogs' order): each kind's titles from every addon ranked together against `query` as
 * `rankResults` ranks sources' (seasons weighed: `videoTitleScore`), the same title from
 * several addons once.
 */
export const mergeKinds = (rows: readonly StremioCatalogRow[], query: string): KindSection[] => {
  const kinds = new Map<string, { kind: string; rows: StremioCatalogRow[] }>();
  for (const row of rows) {
    const key = kindKey(row.kind);
    const found = kinds.get(key);
    if (found) found.rows.push(row);
    else kinds.set(key, { kind: row.kind, rows: [row] });
  }
  return [...kinds].map(([key, { kind, rows: kindRows }]) => ({
    key,
    kind,
    titles: mergeCopies(
      rankLists(
        kindRows.map((row) => row.items),
        query,
        nameOf,
        videoTitleScore,
      ).map(({ item, list }) => ({
        item,
        addonId: kindRows[list]!.addonId,
        addonName: kindRows[list]!.addonName,
      })),
    ),
  }));
};
