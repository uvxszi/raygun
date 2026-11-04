import type { EpisodeFilter, EpisodeOrder, EpisodeSort } from '@raygun/bindings';
import { ArrowDown, ArrowUp, ArrowUpDown, Check } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';

const FILTERS: EpisodeFilter[] = ['unplayed', 'started', 'downloaded'];

const ROW =
  'hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm';

/** Which way a sort starts when picked: newest, longest and unplayed first; A–Z and 1, 2, 3. */
const FIRST_WAY: Record<EpisodeSort, boolean> = {
  date: true,
  number: false,
  title: false,
  duration: true,
  unplayed: false,
};

/**
 * Sort and filter a show's episodes or a book's chapters (M46.48): by date, number, title or
 * length, either way (pick the sort again to turn it round), or unplayed first; and only
 * unplayed, started or downloaded ones. Each show and book keeps its own, in Rust.
 */
export const EpisodeOrderButton: FC<{
  order: EpisodeOrder;
  /** Its order isn't the kind's own. */
  custom: boolean;
  book: boolean;
  onChange: (order: EpisodeOrder | null) => void;
}> = ({ order, custom, book, onChange }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  // Book feeds have no real dates (Loyal Books makes them up), so books don't sort by them.
  const sorts: EpisodeSort[] = book
    ? ['number', 'title', 'duration', 'unplayed']
    : ['date', 'number', 'title', 'duration', 'unplayed'];
  const way = (sort: EpisodeSort, descending: boolean) => {
    if (sort === 'unplayed') return null;
    if (sort === 'date') return t(descending ? 'episodeOrder.newest' : 'episodeOrder.oldest');
    if (sort === 'duration')
      return t(descending ? 'episodeOrder.longest' : 'episodeOrder.shortest');
    if (sort === 'title') return t(descending ? 'episodeOrder.zToA' : 'episodeOrder.aToZ');
    return t(descending ? 'episodeOrder.highest' : 'episodeOrder.lowest');
  };
  return (
    <>
      <button
        type="button"
        aria-label={t('episodeOrder.title')}
        title={t('episodeOrder.title')}
        aria-haspopup="dialog"
        onClick={(event) => setAnchor(event.currentTarget)}
        className={`hover:bg-surface grid size-9 place-items-center rounded-full ${custom ? 'text-primary' : 'text-muted'}`}
      >
        <ArrowUpDown aria-hidden size={18} />
      </button>
      {anchor && (
        <Sheet
          title={t('episodeOrder.title')}
          anchor={anchor}
          popoverWidth={280}
          onClose={() => setAnchor(null)}
        >
          <div className="flex flex-col gap-1 pb-2">
            <p className="text-muted px-3 text-xs font-semibold uppercase">
              {t('episodeOrder.sortBy')}
            </p>
            {sorts.map((sort) => {
              const active = order.sort === sort;
              const Arrow = order.descending ? ArrowDown : ArrowUp;
              const label = way(sort, order.descending);
              return (
                <button
                  key={sort}
                  type="button"
                  aria-pressed={active}
                  onClick={() =>
                    onChange(
                      active
                        ? { ...order, descending: !order.descending }
                        : { ...order, sort, descending: FIRST_WAY[sort] },
                    )
                  }
                  className={`${ROW} ${active ? 'text-primary font-semibold' : ''}`}
                >
                  <span className="grid size-4 place-items-center">
                    {active && sort !== 'unplayed' && <Arrow aria-hidden size={16} />}
                  </span>
                  {t(
                    sort === 'number'
                      ? book
                        ? 'episodeOrder.sort.chapter'
                        : 'episodeOrder.sort.number'
                      : `episodeOrder.sort.${sort}`,
                  )}
                  {active && label && (
                    <span className="text-muted ml-auto text-xs font-normal">{label}</span>
                  )}
                </button>
              );
            })}
            <p className="text-muted mt-2 px-3 text-xs font-semibold uppercase">
              {t('episodeOrder.show')}
            </p>
            {FILTERS.map((filter) => {
              const on = order.filters.includes(filter);
              return (
                <button
                  key={filter}
                  type="button"
                  role="checkbox"
                  aria-checked={on}
                  onClick={() =>
                    onChange({
                      ...order,
                      filters: on
                        ? order.filters.filter((other) => other !== filter)
                        : [...order.filters, filter],
                    })
                  }
                  className={`${ROW} ${on ? 'text-primary' : ''}`}
                >
                  <span
                    className={`grid size-4 place-items-center rounded border ${on ? 'border-primary bg-primary text-background' : 'border-muted'}`}
                  >
                    {on && <Check aria-hidden size={12} />}
                  </span>
                  {t(
                    filter === 'unplayed' && book
                      ? 'episodeOrder.filter.unfinished'
                      : `episodeOrder.filter.${filter}`,
                  )}
                </button>
              );
            })}
            {custom && (
              <button
                type="button"
                onClick={() => onChange(null)}
                className="text-muted hover:text-foreground mt-1 self-start px-3 text-sm underline"
              >
                {t('episodeOrder.reset')}
              </button>
            )}
          </div>
        </Sheet>
      )}
    </>
  );
};
