import type { StremioCatalogInfo, StremioCatalogRow, StremioItem } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { CheckSquare, ChevronRight, Clapperboard } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { CARD_MENU_CLASS, CardPlayButton } from '../manga/CardPlayButton';
import { COVER_GRID_CLASS, coverGridStyle } from '../manga/coverGrid';
import { usePlayTitle, useTitleCardMenu } from '../manga/titleMenu';
import { useSettings } from '../settings/useSettings';
import type { CardTitle } from '../manga/titleMenu';
import { AdultMark } from '../shell/AdultMark';
import { Carousel } from '../shell/Carousel';
import { MenuButton } from '../shell/MenuButton';
import { Picture } from '../shell/Picture';
import { SkeletonSwap } from '../shell/SkeletonSwap';
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

/** A poster in the shape its catalog asks for, fading in, or a placeholder (also when it fails
 * to load); full width unless sized. */
export const StremioPoster: FC<{ url: string | null; shape?: string; className?: string }> = ({
  url,
  shape = 'poster',
  className = 'w-full',
}) => (
  <Picture
    src={url}
    draggable={false}
    placeholder={<Clapperboard size={28} />}
    className={`bg-surface text-muted rounded-lg ${ASPECT[shape] ?? 'aspect-[2/3]'} ${className}`}
  />
);

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

const CardBody: FC<{
  item: StremioItem;
  className?: string;
  actions: CardActions;
  /** Under the card's lines: where it comes from (search). */
  footer?: ReactNode;
}> = ({ item, className = '', actions: { t, menu, play, pickable }, footer }) => {
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
        {footer}
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

/** A card of a list: its title, and what shows under its lines (search: where it's from). */
export type StremioListCard = { key: string; item: StremioItem; footer?: ReactNode };

/** Cards a row draws at once (more than a screen's width), the rest of its 20 just after. */
const FIRST_CARDS = 6;
/** Covers a grid draws at once (a screenful), then as many more as each part is on the screen. */
const FIRST_GRID_CARDS = 24;
/** Wide thumbnails get wider columns (as video sites' in `MangaGrid`). */
const LANDSCAPE_GRID = { gridTemplateColumns: 'repeat(auto-fill, minmax(min(100%, 13rem), 1fr))' };

/**
 * Titles as cards: a row that pages sideways, or a grid of covers sized as the library's are.
 * The cards share the list's menu and play hooks; the first are drawn with the list, the rest
 * just after (`useDrawnCount`).
 */
export const StremioCardList: FC<{ cards: StremioListCard[]; layout?: 'row' | 'grid' }> = ({
  cards,
  layout = 'row',
}) => {
  const actions = useCardActions();
  const coverSize = useSettings().data?.coverSize ?? 0;
  const grid = layout === 'grid';
  const drawn = useDrawnCount(
    cards.length,
    grid ? FIRST_GRID_CARDS : FIRST_CARDS,
    grid ? FIRST_GRID_CARDS : cards.length,
    cards[0]?.key ?? '',
  );
  if (grid) {
    const landscape =
      cards.length > 0 && cards.every((card) => card.item.posterShape === 'landscape');
    return (
      <ul
        className={COVER_GRID_CLASS}
        style={landscape ? LANDSCAPE_GRID : coverGridStyle(coverSize)}
      >
        {cards.slice(0, drawn).map((card) => (
          <li key={card.key}>
            <CardBody item={card.item} actions={actions} footer={card.footer} />
          </li>
        ))}
      </ul>
    );
  }
  return (
    <Carousel className="-mx-4 gap-3 px-4 lg:mx-0 lg:px-0">
      {cards.slice(0, drawn).map((card) => (
        <li
          key={card.key}
          className={card.item.posterShape === 'landscape' ? 'w-48 shrink-0' : 'w-28 shrink-0'}
        >
          <CardBody item={card.item} actions={actions} footer={card.footer} />
        </li>
      ))}
    </Carousel>
  );
};

const Cards: FC<{ items: StremioItem[] }> = ({ items }) => (
  <StremioCardList cards={items.slice(0, 20).map((item) => ({ key: item.id, item }))} />
);

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
  } else {
    body = (
      <SkeletonSwap ready={Boolean(page.data)} skeleton={<Placeholder />}>
        {items.length === 0 ? (
          <p className="text-muted text-sm">{t('watch.catalogEmpty')}</p>
        ) : (
          <Cards items={items} />
        )}
      </SkeletonSwap>
    );
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
