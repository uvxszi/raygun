import { Fragment, useCallback } from 'react';
import type { ReactNode } from 'react';

import { useWindowedRows } from './windowedRows';
import type { RowProps } from './windowedRows';

type VirtualRowsProps<Item> = {
  items: readonly Item[];
  keyOf: (item: Item, index: number) => string;
  /** A row's height before it's drawn, in rem; drawn rows are measured. */
  rowRem: number;
  /** The space between rows, in rem: the same as the list's `gap-*`. */
  gapRem?: number;
  renderRow: (item: Item, index: number, row: RowProps) => ReactNode;
  /** A row to bring onto the screen. */
  showIndex?: number;
  as?: 'ul' | 'ol' | 'div';
  className?: string;
  role?: string;
  'aria-label'?: string;
  'data-testid'?: string;
};

/**
 * A long list that draws only the rows near the screen (past `VIRTUAL_FROM` rows), scrolling
 * with the page (`useWindowedRows`). Rows may differ in height: each is measured once drawn.
 * `renderRow` spreads `row` on its outermost element.
 */
export const VirtualRows = <Item,>({
  items,
  keyOf,
  rowRem,
  gapRem,
  renderRow,
  showIndex,
  as: Tag = 'ul',
  className,
  role,
  'aria-label': label,
  'data-testid': testId,
}: VirtualRowsProps<Item>) => {
  const keyAt = useCallback(
    (index: number) => {
      const item = items[index];
      return item === undefined ? String(index) : keyOf(item, index);
    },
    [items, keyOf],
  );
  const { start, measure, ...rows } = useWindowedRows({
    count: items.length,
    keyOf: keyAt,
    rowRem,
    gapRem,
    showIndex,
  });
  return (
    <Tag
      ref={start}
      className={className}
      role={role}
      aria-label={label}
      data-testid={testId}
      // The rows not drawn keep their room as padding; the page never anchors its scrolling on
      // rows that come and go.
      style={
        rows.windowed
          ? { paddingTop: rows.before, paddingBottom: rows.after, overflowAnchor: 'none' }
          : undefined
      }
    >
      {rows.slice(items).map(([item, index]) => (
        <Fragment key={keyOf(item, index)}>
          {renderRow(item, index, measure ? { ref: measure, 'data-index': index } : {})}
        </Fragment>
      ))}
    </Tag>
  );
};
