import type { StremioCatalogInfo, StremioCatalogRow, StremioItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { CheckSquare, ChevronRight, Clapperboard } from 'lucide-react';
import { useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { CARD_MENU_CLASS, CardPlayButton } from '../manga/CardPlayButton';
import { usePlayTitle, useTitleCardMenu } from '../manga/titleMenu';
import type { CardTitle } from '../manga/titleMenu';
import { AdultMark } from '../shell/AdultMark';
import { Carousel } from '../shell/Carousel';
import { MenuButton } from '../shell/MenuButton';
import { SourceIssue } from '../shell/SourceIssue';
import { SourceTag } from '../shell/SourceTag';
import { useDrawnCount } from '../shell/useDrawnCount';
import { useInView } from '../shell/useInView';
import { Selectable } from '../select/Selectable';
import { useInSelectionScope } from '../select/scopeContext';
import { useSelectionStore } from '../select/selectionStore';
import { useStremioAddons, useStremioRow } from './useStremio';
import { STREMIO_SOURCE } from './useVideoLibrary';

const ASPECT: Record<string, string> = {
  landscape: 'aspect-video',
  square: 'aspect-square',
};

/** A poster in the shape its catalog asks for, or a placeholder (also when it fails to load);
 * full width unless sized. */
export const StremioPoster: FC<{ url: string | null; shape?: string; className?: string }> = ({
  url,
  shape = 'poster',
  className = 'w-full',
}) => {
  const [failed, setFailed] = useState<string | null>(null);
  const aspect = ASPECT[shape] ?? 'aspect-[2/3]';
  return url && failed !== url ? (
    <img
      src={url}
      alt=""
      loading="lazy"
      draggable={false}
      onError={() => setFailed(url)}
      className={`bg-surface rounded-lg object-cover ${aspect} ${className}`}
    />
  ) : (
    <span
      aria-hidden
      className={`bg-surface text-muted grid place-items-center rounded-lg ${aspect} ${className}`}
    >
      <Clapperboard size={28} />
    </span>
  );
};

/** A Stremio item as a card's title (saved ones are `stremio` titles with `<type>:<id>`). */
const stremioCard = (item: StremioItem): CardTitle => ({
  medium: 'video',
  sourceId: STREMIO_SOURCE,
  id: `${item.kind}:${item.id}`,
  title: item.name || item.id,
  coverUrl: item.poster,
  subtitle: item.releaseInfo,
  link: { to: '/watch/title', search: { type: item.kind, id: item.id } },
});

/** What every card of a row shares, worked out once for the row rather than for each card. */
type CardActions = {
  t: TFunction;
  menu: ReturnType<typeof useTitleCardMenu>;
  play: ReturnType<typeof usePlayTitle>;
  /** In search a card can be picked (M46.50). */
  pickable: boolean;
};

const useCardActions = (): CardActions => {
  const { t } = useTranslation();
  const menu = useTitleCardMenu();
  const play = usePlayTitle();
  const pickable = useInSelectionScope();
  return { t, menu, play, pickable };
};

/**
 * A movie, series or channel: the card opens its page, the play button plays it, and ⋮, a right
 * click or a long press opens its full menu.
 */
export const StremioCard: FC<{ item: StremioItem; className?: string }> = ({ item, className }) => {
  const actions = useCardActions();
  return <CardBody item={item} className={className} actions={actions} />;
};

const CardBody: FC<{ item: StremioItem; className?: string; actions: CardActions }> = ({
  item,
  className = '',
  actions: { t, menu, play, pickable },
}) => {
  const name = item.name || item.id;
  // In search a card can be picked (M46.50): a long press, a right click or the circle on its top
  // left picks it, ⋮ on its top right opens the menu (with Select); the box around it then takes
  // the card's size.
  const picked = {
    kind: 'title' as const,
    medium: 'video' as const,
    sourceId: STREMIO_SOURCE,
    remoteId: `${item.kind}:${item.id}`,
    title: name,
  };
  const items = () =>
    menu(
      stremioCard(item),
      pickable
        ? {
            select: {
              label: t('search.select.pick', { title: name }),
              icon: CheckSquare,
              onSelect: () => useSelectionStore.getState().toggle(picked),
            },
          }
        : {},
    );
  return (
    <Selectable picked={picked} className={className}>
      <Link
        to="/watch/title"
        search={{ type: item.kind, id: item.id }}
        aria-label={name}
        data-testid="stremio-card"
        draggable={false}
        className={`flex flex-col gap-1 ${pickable ? '' : className}`}
      >
        <span className="relative">
          <StremioPoster url={item.poster} shape={item.posterShape} />
          <CardPlayButton
            label={t('menu.playTitle', { name })}
            onPlay={() => play(stremioCard(item))}
          />
          <MenuButton
            label={t('menu.options', { name })}
            items={items}
            className={CARD_MENU_CLASS}
            iconSize={16}
          />
        </span>
        {item.name && <span className="line-clamp-2 text-sm font-medium">{item.name}</span>}
        {(item.releaseInfo || item.rating) && (
          <span className="text-muted line-clamp-1 text-xs">
            {[item.releaseInfo, item.rating && `★ ${item.rating}`].filter(Boolean).join(' · ')}
          </span>
        )}
      </Link>
    </Selectable>
  );
};

/** A row's heading: what it lists, which addon, and a way to all of it. */
const RowHeading: FC<{
  title: string;
  addonId: string;
  addonName: string;
  all?: { addon: string; type: string; catalog: string } | null;
}> = ({ title, addonId, addonName, all }) => {
  const { t } = useTranslation();
  const adult = useStremioAddons().data?.some((addon) => addon.id === addonId && addon.adult);
  return (
    <div className="flex items-baseline justify-between gap-2">
      <h2 className="min-w-0 font-semibold">
        <span className="line-clamp-1">{title}</span>
        <span className="flex items-center gap-1.5 font-normal">
          <SourceTag name={addonName} />
          {adult && <AdultMark />}
        </span>
      </h2>
      {all && (
        <Link
          to="/watch/catalog"
          search={all}
          aria-label={t('watch.seeAllOf', { name: title })}
          className="text-primary flex shrink-0 items-center gap-0.5 text-sm"
        >
          {t('search.seeAll')}
          <ChevronRight aria-hidden size={16} />
        </Link>
      )}
    </div>
  );
};

/** Cards a row draws at once (more than a screen's width), the rest of its 20 just after. */
const FIRST_CARDS = 6;

const Cards: FC<{ items: StremioItem[] }> = ({ items }) => {
  const actions = useCardActions();
  const shown = items.slice(0, 20);
  const drawn = useDrawnCount(shown.length, FIRST_CARDS, shown.length, shown[0]?.id ?? '');
  return (
    <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
      {shown.slice(0, drawn).map((item) => (
        <li
          key={item.id}
          className={item.posterShape === 'landscape' ? 'w-48 shrink-0' : 'w-28 shrink-0'}
        >
          <CardBody item={item} actions={actions} />
        </li>
      ))}
    </Carousel>
  );
};

/** Placeholder cards while a row loads, the size of the real ones. */
const Placeholder: FC = () => (
  <ul aria-hidden className="-mx-4 flex gap-3 overflow-hidden px-4 lg:mx-0 lg:px-0">
    {Array.from({ length: 6 }, (_, index) => (
      <li key={index} className="bg-surface aspect-[2/3] w-28 shrink-0 rounded-lg" />
    ))}
  </ul>
);

const rowTitle = (name: string, kind: string) => `${name} · ${kind}`;

/** Search results from one addon's search catalog, whose own name says nothing: named by kind. */
export const StremioRow: FC<{ row: StremioCatalogRow }> = ({ row }) => {
  const { t } = useTranslation();
  const kind = t(`watch.kind.${row.kind}`, { defaultValue: row.kind });
  return (
    <section aria-label={kind} className="flex flex-col gap-2">
      <RowHeading title={kind} addonId={row.addonId} addonName={row.addonName} />
      <Cards items={row.items} />
    </section>
  );
};

/**
 * A catalog as a row (Stremio's board): its first page is asked for once the row comes near the
 * window. On the board (`quiet`) a row with nothing, or whose addon failed, shows nothing at all;
 * on an addon's own page it says so. `keep` narrows its titles (Search's filters).
 */
export const StremioCatalogRowView: FC<{
  catalog: StremioCatalogInfo;
  quiet?: boolean;
  keep?: (items: StremioItem[]) => StremioItem[];
}> = ({ catalog, quiet = false, keep }) => {
  const { t } = useTranslation();
  const [ref, seen] = useInView<HTMLElement>();
  const page = useStremioRow(catalog, seen);
  const kind = t(`watch.kind.${catalog.kind}`, { defaultValue: catalog.kind });
  const title = rowTitle(catalog.name, kind);
  const listed = page.data?.items ?? [];
  const items = keep ? keep(listed) : listed;
  if (quiet && (page.isError || (page.data && items.length === 0))) return null;
  let body: ReactNode;
  if (page.isError) {
    body = (
      <SourceIssue compact source={catalog.addonName} onRetry={() => void page.refetch()}>
        {page.error.message}
      </SourceIssue>
    );
  } else if (page.data) {
    body =
      items.length === 0 ? (
        <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
      ) : (
        <Cards items={items} />
      );
  } else {
    body = <Placeholder />;
  }
  return (
    <section ref={ref} aria-label={title} className="flex flex-col gap-2">
      <RowHeading
        title={title}
        addonId={catalog.addonId}
        addonName={catalog.addonName}
        all={
          items.length > 0
            ? { addon: catalog.addonId, type: catalog.kind, catalog: catalog.catalogId }
            : null
        }
      />
      {body}
    </section>
  );
};
