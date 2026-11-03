import { ArrowDown, ArrowUp, ArrowUpDown, Check } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Sheet } from '../shell/Sheet';
import { DEFAULT_ORDER } from './chapterOrder';
import type { ChapterFilter, ChapterOrder, ChapterSort } from './chapterOrder';

const SORTS: ChapterSort[] = ['source', 'number', 'uploaded'];
const FILTERS: ChapterFilter[] = ['unread', 'started', 'downloaded'];

const ROW =
  'hover:bg-background flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm';

/**
 * Sort and filter a title's chapters (M46.38): by source order, chapter number or upload date,
 * either way (pick the sort again to turn it round), and only unread, started or downloaded ones.
 */
export const ChapterOrderButton: FC<{
  order: ChapterOrder;
  onChange: (order: ChapterOrder) => void;
}> = ({ order, onChange }) => {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const changed =
    order.sort !== DEFAULT_ORDER.sort ||
    order.descending !== DEFAULT_ORDER.descending ||
    order.filters.length > 0;
  return (
    <>
      <button
        type="button"
        aria-label={t('chapterOrder.title')}
        title={t('chapterOrder.title')}
        aria-haspopup="dialog"
        onClick={(event) => setAnchor(event.currentTarget)}
        className={`hover:bg-surface grid size-9 place-items-center rounded-full ${changed ? 'text-primary' : 'text-muted'}`}
      >
        <ArrowUpDown aria-hidden size={18} />
      </button>
      {anchor && (
        <Sheet
          title={t('chapterOrder.title')}
          anchor={anchor}
          popoverWidth={280}
          onClose={() => setAnchor(null)}
        >
          <div className="flex flex-col gap-1 pb-2">
            <p className="text-muted px-3 text-xs font-semibold uppercase">
              {t('chapterOrder.sortBy')}
            </p>
            {SORTS.map((sort) => {
              const active = order.sort === sort;
              const Arrow = order.descending ? ArrowDown : ArrowUp;
              return (
                <button
                  key={sort}
                  type="button"
                  aria-pressed={active}
                  onClick={() =>
                    onChange(
                      active
                        ? { ...order, descending: !order.descending }
                        : { ...order, sort, descending: true },
                    )
                  }
                  className={`${ROW} ${active ? 'text-primary font-semibold' : ''}`}
                >
                  <span className="grid size-4 place-items-center">
                    {active && <Arrow aria-hidden size={16} />}
                  </span>
                  {t(`chapterOrder.sort.${sort}`)}
                  {active && (
                    <span className="text-muted ml-auto text-xs font-normal">
                      {sort === 'source'
                        ? t(order.descending ? 'chapterOrder.asListed' : 'chapterOrder.reversed')
                        : t(
                            order.descending ? 'chapterOrder.descending' : 'chapterOrder.ascending',
                          )}
                    </span>
                  )}
                </button>
              );
            })}
            <p className="text-muted mt-2 px-3 text-xs font-semibold uppercase">
              {t('chapterOrder.show')}
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
                  {t(`chapterOrder.filter.${filter}`)}
                </button>
              );
            })}
            {changed && (
              <button
                type="button"
                onClick={() => onChange(DEFAULT_ORDER)}
                className="text-muted hover:text-foreground mt-1 self-start px-3 text-sm underline"
              >
                {t('chapterOrder.reset')}
              </button>
            )}
          </div>
        </Sheet>
      )}
    </>
  );
};
