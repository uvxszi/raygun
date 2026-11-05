import type { TapCell } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  DEFAULT_GRIDS,
  gridOf,
  TAP_CELLS,
  TAP_PRESET_NAMES,
  TAP_PRESETS,
  toGrid,
} from './reader/tapZones';
import type { GridKind, TapGridCells } from './reader/tapZones';
import { useReaderDisplay } from './readerDisplay';

/** Each action's colour in the preview. */
const CELL_COLORS: Record<TapCell, string> = {
  menu: 'bg-neutral-500/25',
  previous: 'bg-sky-500/45',
  next: 'bg-emerald-500/45',
  left: 'bg-amber-500/45',
  right: 'bg-violet-500/45',
  firstPage: 'bg-rose-500/45',
  lastPage: 'bg-pink-500/45',
  previousChapter: 'bg-orange-500/45',
  nextChapter: 'bg-teal-500/45',
  nothing: 'border border-dashed border-current/30',
};

const KINDS: GridKind[] = ['paged', 'webtoon'];

/**
 * The custom tap zones: a 3×3 preview of a page for the paged modes and for the strip. Tapping a
 * cell selects it, and the action chips set what it does; presets and Reset start the grid over.
 */
export const TapGridEditor: FC = () => {
  const { t } = useTranslation();
  const { display, set, busy } = useReaderDisplay();
  const [kind, setKind] = useState<GridKind>('paged');
  const [selected, setSelected] = useState(4);
  const grid = gridOf(display, kind);
  const actionLabel = (cell: TapCell) => t(`reader.display.tapGrid.action.${cell}`);

  // Both grids are written, so the mode not being edited keeps what it does now.
  const save = (next: TapGridCells) =>
    set({
      tapZones: 'custom',
      tapGridPaged: toGrid(kind === 'paged' ? next : gridOf(display, 'paged')),
      tapGridWebtoon: toGrid(kind === 'webtoon' ? next : gridOf(display, 'webtoon')),
    });
  const setCell = (action: TapCell) =>
    save(grid.map((cell, index) => (index === selected ? action : cell)));

  const chip = (pressed: boolean) =>
    `rounded-full px-3 py-1.5 text-sm ${pressed ? 'bg-primary text-background font-semibold' : 'bg-background'}`;

  return (
    <div data-testid="tap-grid-editor" className="flex flex-col gap-3 px-1 py-2">
      <div className="flex gap-2">
        {KINDS.map((option) => (
          <button
            key={option}
            type="button"
            aria-pressed={kind === option}
            onClick={() => setKind(option)}
            className={chip(kind === option)}
          >
            {t(`reader.display.tapGrid.${option}`)}
          </button>
        ))}
      </div>
      <div
        role="group"
        aria-label={t('reader.display.tapGrid.label', {
          mode: t(`reader.display.tapGrid.${kind}`),
        })}
        className="border-muted/40 mx-auto grid aspect-[9/16] w-40 grid-cols-3 grid-rows-3 gap-1 rounded-2xl border-4 p-1"
      >
        {grid.map((cell, index) => (
          <button
            // By place: the grid's cells never move.
            key={index}
            type="button"
            aria-pressed={selected === index}
            aria-label={t('reader.display.tapGrid.cell', {
              row: Math.floor(index / 3) + 1,
              column: (index % 3) + 1,
              action: actionLabel(cell),
            })}
            onClick={() => setSelected(index)}
            className={`grid place-items-center rounded-md p-0.5 text-center text-[calc(0.65rem*var(--text-scale,1))] leading-tight ${CELL_COLORS[cell]} ${selected === index ? 'ring-primary ring-2' : ''}`}
          >
            {actionLabel(cell)}
          </button>
        ))}
      </div>
      <div
        role="radiogroup"
        aria-label={t('reader.display.tapGrid.actions')}
        className="flex flex-wrap gap-2"
      >
        {TAP_CELLS.map((action) => (
          <button
            key={action}
            type="button"
            role="radio"
            aria-checked={grid[selected] === action}
            disabled={busy}
            onClick={() => setCell(action)}
            className={chip(grid[selected] === action)}
          >
            {actionLabel(action)}
          </button>
        ))}
      </div>
      <div
        role="group"
        aria-label={t('reader.display.tapGrid.presets')}
        className="flex flex-wrap gap-2"
      >
        {TAP_PRESET_NAMES.map((preset) => (
          <button
            key={preset}
            type="button"
            disabled={busy}
            onClick={() => save(TAP_PRESETS[preset])}
            className={chip(false)}
          >
            {t(`reader.display.tapZone.${preset}`)}
          </button>
        ))}
        <button
          type="button"
          disabled={busy}
          onClick={() => save(DEFAULT_GRIDS[kind])}
          className={chip(false)}
        >
          {t('reader.display.tapGrid.reset')}
        </button>
      </div>
    </div>
  );
};
