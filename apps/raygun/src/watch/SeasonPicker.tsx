import { Check, ChevronDown } from 'lucide-react';
import { useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MENU_ROW } from '../shell/menuRow';
import { Sheet } from '../shell/Sheet';
import { useDragScroll } from '../shell/useDragScroll';

/** A season's number; specials are `0` or `null` (a source's episodes without one). */
export type SeasonValue = number | null;

/** More seasons than this pick from a list instead of a row of pills. */
export const SEASON_PILLS_MAX = 6;

type Props = {
  seasons: SeasonValue[];
  shown: SeasonValue;
  onPick: (season: SeasonValue) => void;
  /** A season's name ("Season 2", "Specials"). */
  label: (season: SeasonValue) => string;
};

const key = (season: SeasonValue) => (season === null ? 'none' : String(season));

/**
 * A series' seasons: a row of pills while they're few and fit on one line, else a button naming
 * the season shown that opens a list of them (a popover beside it on wide windows).
 */
export const SeasonPicker: FC<Props> = ({ seasons, shown, onPick, label }) => {
  const { t } = useTranslation();
  const row = useRef<HTMLDivElement | null>(null);
  const dragRef = useDragScroll(row);
  // Pills that overflow their row (long names, a narrow window) become a list too.
  const [overflows, setOverflows] = useState(false);
  const [open, setOpen] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const many = seasons.length > SEASON_PILLS_MAX;
  const asList = many || overflows;

  useLayoutEffect(() => {
    const element = row.current;
    if (!element || asList) return;
    const measure = () => setOverflows(element.scrollWidth > element.clientWidth + 1);
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [asList, seasons]);

  if (asList) {
    return (
      <>
        <button
          ref={setAnchor}
          type="button"
          aria-haspopup="dialog"
          aria-expanded={open}
          aria-label={t('watch.pickSeason', { season: label(shown) })}
          onClick={() => setOpen((was) => !was)}
          className="bg-primary/20 text-primary flex items-center gap-1.5 self-start rounded-full px-4 py-1.5 text-sm font-semibold"
        >
          {label(shown)}
          <ChevronDown aria-hidden size={16} />
        </button>
        {open && (
          <Sheet title={t('watch.seasons')} anchor={anchor} onClose={() => setOpen(false)}>
            <div role="listbox" aria-label={t('watch.seasons')} className="flex flex-col pb-2">
              {seasons.map((season) => (
                <button
                  key={key(season)}
                  type="button"
                  role="option"
                  aria-selected={season === shown}
                  onClick={() => {
                    onPick(season);
                    setOpen(false);
                  }}
                  className={`hover:bg-surface flex items-center justify-between gap-3 px-4 text-left ${MENU_ROW} ${season === shown ? 'text-primary font-semibold' : ''}`}
                >
                  {label(season)}
                  {season === shown && <Check aria-hidden size={18} />}
                </button>
              ))}
            </div>
          </Sheet>
        )}
      </>
    );
  }

  return (
    <div
      ref={dragRef}
      role="tablist"
      aria-label={t('watch.seasons')}
      className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
    >
      {seasons.map((season) => (
        <button
          key={key(season)}
          type="button"
          role="tab"
          aria-selected={season === shown}
          onClick={() => onPick(season)}
          className={`shrink-0 rounded-full px-4 py-1.5 text-sm ${season === shown ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
        >
          {label(season)}
        </button>
      ))}
    </div>
  );
};
