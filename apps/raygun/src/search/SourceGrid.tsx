import { Check, ChevronRight, Minus } from 'lucide-react';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties, FC, KeyboardEvent, MouseEvent } from 'react';

import { useRootPx } from '../player/playerLayout';
import { rowsEqual, useWindowedRows } from '../shell/windowedRows';
import type { RowProps } from '../shell/windowedRows';
import { cellsOf, columnsFor, gridRows, moveFocus } from './customGrid';
import type { Cell, GridGroup, GridRow, GridSource } from './customGrid';
import { SourceTile } from './SourceTile';

/** A row of tiles before it's measured, and the space between rows (`gap-1.5`), in rem. */
const ROW_REM = 5;
const GAP_REM = 0.375;
/** Grids of more rows than this draw only the rows near the panel's view (it shows about 4). */
const VIRTUAL_FROM = 10;
const OVERSCAN = 4;
/** The panel's height for the grid, at most (`max-h-[min(24rem,50dvh)]`). */
const VIEW_REM = 24;
/** Rows Page Up and Page Down move. */
const PAGE_ROWS = 4;
/** Keys the grid answers; others (Space and Enter press the tile) go on as they would. */
const MOVES = new Set(['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'PageDown', 'PageUp']);
const ENDS = new Set(['Home', 'End']);

/** The grid scrolls in its own box, not with the page. */
const ownScroller = (list: HTMLElement) => list.parentElement;

/** An element's width, followed as it changes: the grid's columns follow the panel's. */
const useWidth = (element: HTMLElement | null) => {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    if (!element) return;
    const measure = () => setWidth(element.clientWidth);
    measure();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [element]);
  return width;
};

/** A cell's lasting name, so the focus follows a tile when the rows change. */
const cellKey = (row: GridRow, col: number) =>
  row.kind === 'tiles' ? `tile:${row.tiles[col]?.id ?? ''}` : `${col}:${row.group.key}`;

const cellAt = (scroller: HTMLElement | null, cell: Cell) =>
  scroller?.querySelector<HTMLElement>(`[data-row="${cell.row}"][data-col="${cell.col}"]`) ?? null;

type TileRowProps = {
  row: RowProps;
  index: number;
  tiles: readonly GridSource[];
  /** Each tile on (`1`) or off (`0`), so a row that didn't change isn't drawn again. */
  mask: string;
  /** The tile that's the grid's stop for Tab, if it's in this row. */
  tabCol: number;
  adultMark: string;
  adultLabel: string;
};

const TileRow = memo<TileRowProps>(
  ({ row, index, tiles, mask, tabCol, adultMark, adultLabel }) => (
    <div {...row} className="grid grid-cols-[repeat(var(--source-columns),minmax(0,1fr))] gap-1.5">
      {tiles.map((tile, col) => (
        <SourceTile
          key={tile.id}
          id={tile.id}
          name={tile.name}
          icon={tile.icon}
          adultMark={tile.adult ? adultMark : null}
          adultLabel={tile.adult ? adultLabel : null}
          on={mask[col] === '1'}
          row={index}
          col={col}
          tabStop={col === tabCol}
        />
      ))}
    </div>
  ),
  rowsEqual,
);

type GroupRowProps = {
  row: RowProps;
  index: number;
  group: string;
  label: string;
  /** The group's sources on, of `total`. */
  count: number;
  total: number;
  open: boolean;
  tabCol: number;
  switchLabel: string;
};

/** A group's header: its name and count fold it; the box beside switches all of it. */
const GroupRow = memo<GroupRowProps>(
  ({ row, index, group, label, count, total, open, tabCol, switchLabel }) => {
    const all = count === total;
    return (
      <div {...row} className="flex items-center gap-1 pt-1">
        <button
          type="button"
          aria-expanded={open}
          tabIndex={tabCol === 0 ? 0 : -1}
          data-cell=""
          data-act="fold"
          data-group={group}
          data-row={index}
          data-col={0}
          className="focus-visible:outline-primary flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 pr-2 text-left text-sm font-semibold focus-visible:outline-2"
        >
          <ChevronRight
            aria-hidden
            size={16}
            className={`text-muted shrink-0 transition-transform ${open ? 'rotate-90' : ''}`}
          />
          <span className="truncate">{label}</span>
          <span className="text-muted shrink-0 text-xs font-normal tabular-nums">
            {count}/{total}
          </span>
        </button>
        <button
          type="button"
          role="checkbox"
          aria-checked={all ? true : count === 0 ? false : 'mixed'}
          aria-label={switchLabel}
          tabIndex={tabCol === 1 ? 0 : -1}
          data-cell=""
          data-act="all"
          data-group={group}
          data-row={index}
          data-col={1}
          className="focus-visible:outline-primary grid size-8 shrink-0 place-items-center rounded-md focus-visible:outline-2"
        >
          <span
            aria-hidden
            className={`grid size-4.5 place-items-center rounded border-2 ${
              all
                ? 'bg-primary border-primary text-background'
                : count > 0
                  ? 'border-primary text-primary'
                  : 'border-muted/60'
            }`}
          >
            {all ? (
              <Check size={11} strokeWidth={3.5} />
            ) : count > 0 ? (
              <Minus size={11} strokeWidth={3.5} />
            ) : null}
          </span>
        </button>
      </div>
    );
  },
  rowsEqual,
);

type SourceGridProps = {
  groups: readonly GridGroup[];
  /** Groups have headers that fold them (more than one group). */
  headers: boolean;
  folded: ReadonlySet<string>;
  /** The sources switched on. */
  on: ReadonlySet<string>;
  adultMark: string;
  adultLabel: string;
  /** What a group's switch says to a screen reader, for the group's name. */
  groupSwitchLabel: (name: string) => string;
  onToggle: (id: string) => void;
  onFold: (group: string) => void;
  /** A group's switch: every source in it on, or (all on already) off. */
  onGroup: (group: string) => void;
};

/**
 * Custom sources' tiles, as many a row as the panel's width takes (4, 6 or 8), in a box of its
 * own height that scrolls. Long grids draw only the rows near the box's view. One listener
 * answers every tile's click and key: the arrows move between tiles (and headers), Space and
 * Enter switch the one focused, Tab leaves the grid.
 */
export const SourceGrid: FC<SourceGridProps> = ({
  groups,
  headers,
  folded,
  on,
  adultMark,
  adultLabel,
  groupSwitchLabel,
  onToggle,
  onFold,
  onGroup,
}) => {
  const [scroller, setScroller] = useState<HTMLDivElement | null>(null);
  const columns = columnsFor(useWidth(scroller), useRootPx());
  const rows = useMemo(
    () => gridRows(groups, columns, folded, headers),
    [groups, columns, folded, headers],
  );
  const keyAt = useCallback((index: number) => rows[index]?.key ?? String(index), [rows]);
  const { start, measure, ...windowing } = useWindowedRows({
    count: rows.length,
    keyOf: keyAt,
    rowRem: ROW_REM,
    gapRem: GAP_REM,
    virtualFrom: VIRTUAL_FROM,
    overscan: OVERSCAN,
    scrollerOf: ownScroller,
    viewRem: VIEW_REM,
  });
  // The cell the keys last moved to: the grid's stop for Tab while it's drawn.
  const [active, setActive] = useState<string | null>(null);
  const moved = useRef<string | null>(null);
  // The column Up and Down keep, as in a table.
  const column = useRef(0);
  // A cell moved to before its row was drawn: focused once it is.
  const pending = useRef<Cell | null>(null);

  const places = useMemo(() => {
    const found = new Map<string, Cell>();
    rows.forEach((row, index) => {
      for (let col = 0; col < cellsOf(row); col++)
        found.set(cellKey(row, col), { row: index, col });
    });
    return found;
  }, [rows]);

  useEffect(() => {
    const want = pending.current;
    const element = want && cellAt(scroller, want);
    if (element) {
      pending.current = null;
      element.focus();
    }
  });

  const drawn = windowing.slice(rows);
  const activeAt = active ? places.get(active) : undefined;
  const tabAt =
    activeAt && drawn.some(([, index]) => index === activeAt.row)
      ? activeAt
      : drawn[0]
        ? { row: drawn[0][1], col: 0 }
        : null;

  const focusCell = (to: Cell) => {
    const row = rows[to.row];
    if (!row) return;
    const key = cellKey(row, to.col);
    moved.current = key;
    setActive(key);
    const element = cellAt(scroller, to);
    if (element) {
      element.focus();
    } else {
      pending.current = to;
      windowing.scrollToRow(to.row);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const moves = MOVES.has(event.key) || ENDS.has(event.key);
    // Shift and Ctrl with the arrows are the player's (seek, next song).
    if (!moves || event.altKey || event.metaKey || event.shiftKey) return;
    if (event.ctrlKey && !ENDS.has(event.key)) return;
    const cell = (event.target as HTMLElement).closest<HTMLElement>('[data-cell]');
    const at = cell && { row: Number(cell.dataset.row), col: Number(cell.dataset.col) };
    const here = at && rows[at.row];
    if (!at || !here) return;
    event.preventDefault();
    // Come by a tap, a click or Tab: Up and Down keep this column.
    if (moved.current !== cellKey(here, at.col)) column.current = at.col;
    const to = moveFocus(rows, at, event.key, event.ctrlKey, column.current, PAGE_ROWS);
    if (!to) return;
    if (!/^(ArrowUp|ArrowDown|PageUp|PageDown)$/.test(event.key)) column.current = to.col;
    focusCell(to);
  };

  const onClick = (event: MouseEvent<HTMLDivElement>) => {
    const cell = (event.target as HTMLElement).closest<HTMLElement>('[data-cell]');
    if (!cell) return;
    const { source, group, act } = cell.dataset;
    if (source !== undefined) onToggle(source);
    else if (group !== undefined && act === 'fold') onFold(group);
    else if (group !== undefined && act === 'all') onGroup(group);
  };

  return (
    <div
      ref={setScroller}
      data-testid="source-grid"
      data-columns={columns}
      onClick={onClick}
      onKeyDown={onKeyDown}
      style={{ '--source-columns': columns } as CSSProperties}
      className="-mx-1 max-h-[min(24rem,50dvh)] overflow-y-auto px-1 py-1"
    >
      <div
        ref={start}
        className="flex flex-col gap-1.5"
        // The rows not drawn keep their room as padding.
        style={
          windowing.windowed
            ? {
                paddingTop: windowing.before,
                paddingBottom: windowing.after,
                overflowAnchor: 'none',
              }
            : undefined
        }
      >
        {drawn.map(([row, index]) => {
          const props: RowProps = measure ? { ref: measure, 'data-index': index } : {};
          const tabCol = tabAt?.row === index ? tabAt.col : -1;
          if (row.kind === 'group') {
            const { group } = row;
            return (
              <GroupRow
                key={row.key}
                row={props}
                index={index}
                group={group.key}
                label={group.label}
                count={group.sources.reduce(
                  (count, source) => count + Number(on.has(source.id)),
                  0,
                )}
                total={group.sources.length}
                open={row.open}
                tabCol={tabCol}
                switchLabel={groupSwitchLabel(group.label)}
              />
            );
          }
          return (
            <TileRow
              key={row.key}
              row={props}
              index={index}
              tiles={row.tiles}
              mask={row.tiles.map((tile) => (on.has(tile.id) ? '1' : '0')).join('')}
              tabCol={tabCol}
              adultMark={adultMark}
              adultLabel={adultLabel}
            />
          );
        })}
      </div>
    </div>
  );
};
