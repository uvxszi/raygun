import type { TableCell } from '@raygun/bindings';
import type { FC } from 'react';

import type { TableBlock as TableData } from './spoken';
import { TextParts } from './TextParts';
import './bookText.css';

const Cell: FC<{ cell: TableCell; inHead: boolean }> = ({ cell, inHead }) => {
  const parts = <TextParts parts={cell.parts} />;
  const spans = { colSpan: cell.colspan ?? undefined, rowSpan: cell.rowspan ?? undefined };
  if (!cell.header) return <td {...spans}>{parts}</td>;
  return (
    <th {...spans} scope={inHead ? 'col' : 'row'}>
      {parts}
    </th>
  );
};

/**
 * A table of data in the book reader: as wide as its cells need and no wider than the column; a
 * wider one scrolls sideways in its own box (bookText.css). The first rows that are all headers
 * are its head.
 */
export const TableBlock: FC<{
  block: TableData;
  index: number;
  /** Being read aloud. */
  active: boolean;
  register: (index: number, element: HTMLElement | null) => void;
}> = ({ block, index, active, register }) => {
  const firstBody = block.rows.findIndex((row) => row.some((cell) => !cell.header));
  const headRows = firstBody < 0 ? 0 : firstBody;
  const head = block.rows.slice(0, headRows);
  const body = block.rows.slice(headRows);
  return (
    <div
      ref={(element) => register(index, element)}
      data-block={index}
      aria-current={active || undefined}
      className={`book-table ${active ? 'bg-primary/15 rounded' : ''}`}
    >
      <table>
        {block.caption && block.caption.length > 0 && (
          <caption>
            <TextParts parts={block.caption} />
          </caption>
        )}
        {head.length > 0 && (
          <thead>
            {head.map((row, rowIndex) => (
              <tr key={rowIndex}>
                {row.map((cell, cellIndex) => (
                  <Cell key={cellIndex} cell={cell} inHead />
                ))}
              </tr>
            ))}
          </thead>
        )}
        <tbody>
          {body.map((row, rowIndex) => (
            <tr key={rowIndex}>
              {row.map((cell, cellIndex) => (
                <Cell key={cellIndex} cell={cell} inHead={false} />
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};
