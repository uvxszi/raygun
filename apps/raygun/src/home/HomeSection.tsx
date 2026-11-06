import { Link } from '@tanstack/react-router';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { Carousel } from '../shell/Carousel';
import { MenuButton } from '../shell/MenuButton';
import { useHomeRowActions } from './homeRows';
import type { RowLink } from './homeRows';

type HomeSectionProps = {
  /** Its id in Home's layout: Hide this row hides it. */
  id: string;
  title: string;
  /** Where the title and Show all lead. */
  all?: RowLink;
  /** The source the row comes from, small beside the title. */
  source?: string | null;
  /** The heading's own buttons (Clear history), before Show all. */
  extra?: ReactNode;
  testId?: string;
  children: ReactNode;
};

/**
 * One of Home's rows: its title (leading to all of it), Show all, and ⋯ with Hide this row and
 * Edit Home (also on a right click or long press on the heading), then its cards.
 */
export const HomeSection: FC<HomeSectionProps> = ({
  id,
  title,
  all,
  source,
  extra,
  testId,
  children,
}) => {
  const { t } = useTranslation();
  const rowMenu = useHomeRowActions();
  return (
    <section
      aria-label={title}
      data-testid={testId}
      data-home-section={id}
      className="group/section flex flex-col gap-1"
    >
      <div className="flex min-h-9 items-center gap-3">
        <h2 className="min-w-0 truncate text-lg font-semibold">
          {all ? (
            <Link to={all.to} search={all.search as never} className="hover:underline">
              {title}
            </Link>
          ) : (
            title
          )}
        </h2>
        {source && <span className="text-muted min-w-0 shrink-[3] truncate text-xs">{source}</span>}
        <span className="ml-auto flex shrink-0 items-center gap-1">
          {extra}
          {all && (
            <Link
              to={all.to}
              search={all.search as never}
              aria-label={t('home.showAllOf', { name: title })}
              className="text-muted hover:text-foreground px-1 text-sm font-semibold"
            >
              {t('discover.seeAll')}
            </Link>
          )}
          <MenuButton
            horizontal
            label={t('menu.more', { name: title })}
            items={() => rowMenu(id, title)}
            className="text-muted hover:text-foreground hover:bg-surface grid size-8 place-items-center rounded-full focus-visible:opacity-100 group-hover/section:opacity-100 [@media(hover:hover)]:opacity-0"
          />
        </span>
      </div>
      {children}
    </section>
  );
};

/** A row of cards that pages sideways, edge to edge on phones. */
export const HomeCarousel: FC<{ children: ReactNode; tight?: boolean }> = ({
  children,
  tight = false,
}) => (
  <Carousel className={tight ? '-mx-4 gap-1 px-2 lg:-mx-2' : '-mx-4 gap-3 px-4 lg:mx-0 lg:px-0'}>
    {children}
  </Carousel>
);

/** Placeholders the size of a row's cards, while it loads, so nothing jumps when it comes. */
export const RowSkeleton: FC<{
  /** The cards' width classes, as the real ones. */
  card?: string;
  /** Round (artists), 2:3 (comic and book covers) or 16:9 (videos) instead of square. */
  shape?: 'square' | 'round' | 'poster' | 'wide';
  count?: number;
}> = ({ card = 'w-36 sm:w-40', shape = 'square', count = 6 }) => (
  <HomeCarousel tight>
    {Array.from({ length: count }, (_, index) => (
      <li key={index} aria-hidden className={`${card} shrink-0 p-2`}>
        <span
          className={`bg-surface block animate-pulse ${
            shape === 'round'
              ? 'aspect-square rounded-full'
              : shape === 'poster'
                ? 'aspect-[2/3] rounded-xl'
                : shape === 'wide'
                  ? 'aspect-video rounded-xl'
                  : 'aspect-square rounded-lg'
          }`}
        />
        <span className="bg-surface mt-2 block h-3 w-3/4 animate-pulse rounded" />
      </li>
    ))}
  </HomeCarousel>
);
