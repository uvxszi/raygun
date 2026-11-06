import type { StremioCatalogInfo, StremioExtraValue } from '@raygun/bindings';
import { useNavigate, useSearch } from '@tanstack/react-router';
import { LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { StremioCard } from '../watch/StremioRow';
import { useStremioCatalog, useStremioCatalogs } from '../watch/useStremio';
import { PageHeader } from './PageHeader';
import { LoadMore } from '../shell/LoadMore';
import { SourceIssue } from '../shell/SourceIssue';
import { useDragScroll } from '../shell/useDragScroll';

/** Types in Stremio's order (`TYPE_PRIORITIES`); others after, as the addons list them. */
const TYPE_ORDER = ['movie', 'series', 'channel', 'tv'];
/** Cards drawn at a time: some catalogs send a thousand channels on one page. */
const CHUNK = 120;

type DiscoverSearch = {
  addon?: string;
  type?: string;
  catalog?: string;
  genre?: string;
  extra?: string;
};

/** The extras chosen, from the address: `genre`, and the others as a query string. */
const chosenExtras = (search: DiscoverSearch): StremioExtraValue[] => {
  const values: StremioExtraValue[] = search.genre ? [{ name: 'genre', value: search.genre }] : [];
  for (const [name, value] of new URLSearchParams(search.extra ?? '')) {
    if (name !== 'genre' && value) values.push({ name, value });
  }
  return values;
};

/** What the catalog gets: the choices it knows, a required extra's first option by default. */
const effectiveExtras = (
  catalog: StremioCatalogInfo,
  chosen: StremioExtraValue[],
): StremioExtraValue[] =>
  catalog.extras.flatMap((extra) => {
    const value = chosen.find(
      (choice) => choice.name === extra.name && extra.options.includes(choice.value),
    )?.value;
    if (value) return [{ name: extra.name, value }];
    return extra.required && extra.options[0]
      ? [{ name: extra.name, value: extra.options[0] }]
      : [];
  });

const chip = (on: boolean) =>
  `shrink-0 rounded-full px-4 py-1.5 text-sm ${on ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`;

const ChipRow: FC<{ label: string; children: ReactNode }> = ({ label, children }) => {
  const row = useDragScroll<HTMLDivElement>();
  return (
    <div
      ref={row}
      role="group"
      aria-label={label}
      className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
    >
      {children}
    </div>
  );
};

/** One catalog's items as a grid, page after page. */
const CatalogGrid: FC<{ catalog: StremioCatalogInfo; extra: StremioExtraValue[] }> = ({
  catalog,
  extra,
}) => {
  const { t } = useTranslation();
  const pages = useStremioCatalog(catalog.addonId, catalog.kind, catalog.catalogId, extra);
  const [shown, setShown] = useState(CHUNK);
  const items = pages.data?.pages.flatMap((page) => page.items) ?? [];
  // Some addons repeat items across pages.
  const seen = new Set<string>();
  const unique = items.filter((item) => !seen.has(item.id) && seen.add(item.id));
  const landscape = unique[0]?.posterShape === 'landscape';
  const more = shown < unique.length;
  return (
    <>
      {pages.error && (
        <SourceIssue source={catalog.addonName} onRetry={() => void pages.refetch()}>
          {pages.error.message}
        </SourceIssue>
      )}
      {pages.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {pages.data && unique.length === 0 && (
        <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
      )}
      <ul
        aria-label={catalog.name}
        className={`grid gap-3 ${landscape ? 'grid-cols-2 sm:grid-cols-3' : 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6'}`}
      >
        {unique.slice(0, shown).map((item) => (
          <li key={item.id}>
            <StremioCard item={item} />
          </li>
        ))}
      </ul>
      <LoadMore
        hasNextPage={more || pages.hasNextPage}
        fetching={pages.isFetchingNextPage}
        onLoad={() => {
          if (more) setShown(shown + CHUNK);
          else void pages.fetchNextPage();
        }}
        label={t('search.more')}
      />
    </>
  );
};

/**
 * Discover (Stremio's): every addon's catalogs by type, then by catalog, narrowed by its choices
 * (genres, years...), page after page. A catalog that needs a choice starts on its first one.
 */
export const WatchCatalogPage: FC = () => {
  const { t } = useTranslation();
  const search = useSearch({ from: '/watch/catalog' });
  const navigate = useNavigate({ from: '/watch/catalog' });
  const catalogs = useStremioCatalogs();
  const browsable = (catalogs.data ?? []).filter((catalog) => catalog.discoverable);
  const types = [...new Set(browsable.map((catalog) => catalog.kind))].sort(
    (a, b) =>
      (TYPE_ORDER.indexOf(a) + 1 || TYPE_ORDER.length + 1) -
      (TYPE_ORDER.indexOf(b) + 1 || TYPE_ORDER.length + 1),
  );
  const type = search.type && types.includes(search.type) ? search.type : (types[0] ?? '');
  const ofType = browsable.filter((catalog) => catalog.kind === type);
  const chosen = chosenExtras(search);
  // Without a catalog named, a genre picks the first catalog that has it.
  const offers = (catalog: StremioCatalogInfo) =>
    chosen.every((choice) =>
      catalog.extras.some(
        (extra) => extra.name === choice.name && extra.options.includes(choice.value),
      ),
    );
  const selected =
    ofType.find(
      (catalog) => catalog.addonId === search.addon && catalog.catalogId === search.catalog,
    ) ??
    ofType.find(offers) ??
    ofType[0];
  const extra = selected ? effectiveExtras(selected, chosen) : [];
  const kind = (value: string) => t(`watch.kind.${value}`, { defaultValue: value });

  const go = (next: DiscoverSearch) => void navigate({ search: next, replace: true });
  const choose = (name: string, value: string | null) => {
    const others = extra.filter((choice) => choice.name !== name);
    const values = value ? [...others, { name, value }] : others;
    const genre = values.find((choice) => choice.name === 'genre')?.value;
    const rest = new URLSearchParams(
      values
        .filter((choice) => choice.name !== 'genre')
        .map((choice) => [choice.name, choice.value]),
    ).toString();
    go({
      type,
      addon: selected?.addonId,
      catalog: selected?.catalogId,
      ...(genre ? { genre } : {}),
      ...(rest ? { extra: rest } : {}),
    });
  };

  return (
    <section data-testid="watch-catalog-page" className="flex flex-col gap-4">
      <PageHeader
        back
        fallback="/browse"
        title={t('watch.discover')}
        subtitle={selected ? `${selected.name} · ${selected.addonName}` : undefined}
      />
      {catalogs.error && <SourceIssue>{catalogs.error.message}</SourceIssue>}
      {catalogs.isPending && (
        <LoaderCircle aria-label={t('watch.loading')} className="text-muted mx-auto animate-spin" />
      )}
      {catalogs.data && browsable.length === 0 && (
        <p className="text-muted text-sm">{t('watch.noCatalogs')}</p>
      )}
      {types.length > 1 && (
        <ChipRow label={t('watch.types')}>
          {types.map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={value === type}
              onClick={() => go({ type: value })}
              className={chip(value === type)}
            >
              {kind(value)}
            </button>
          ))}
        </ChipRow>
      )}
      {ofType.length > 1 && (
        <ChipRow label={t('watch.catalogs')}>
          {ofType.map((catalog) => {
            const on = catalog === selected;
            return (
              <button
                key={`${catalog.addonId}-${catalog.catalogId}`}
                type="button"
                aria-pressed={on}
                onClick={() => go({ type, addon: catalog.addonId, catalog: catalog.catalogId })}
                className={chip(on)}
              >
                {catalog.name}
                <span className={on ? 'font-normal' : 'text-muted'}> · {catalog.addonName}</span>
              </button>
            );
          })}
        </ChipRow>
      )}
      {selected?.extras.map((option) => {
        const current = extra.find((choice) => choice.name === option.name)?.value ?? null;
        const label =
          option.name === 'genre'
            ? t('watch.genres')
            : option.name.charAt(0).toUpperCase() + option.name.slice(1);
        return (
          <ChipRow key={option.name} label={label}>
            {!option.required && (
              <button
                type="button"
                aria-pressed={current === null}
                onClick={() => choose(option.name, null)}
                className={chip(current === null)}
              >
                {t('watch.allGenres')}
              </button>
            )}
            {option.options.map((value) => (
              <button
                key={value}
                type="button"
                aria-pressed={value === current}
                onClick={() => choose(option.name, value)}
                className={chip(value === current)}
              >
                {value}
              </button>
            ))}
          </ChipRow>
        );
      })}
      {selected && (
        <CatalogGrid
          key={`${selected.addonId}-${selected.kind}-${selected.catalogId}-${JSON.stringify(extra)}`}
          catalog={selected}
          extra={extra}
        />
      )}
    </section>
  );
};
