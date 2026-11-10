import { render, screen, within } from '@testing-library/react';

import { VirtualRows } from './VirtualRows';
import { VIRTUAL_FROM } from './windowedRows';

const names = (count: number) => Array.from({ length: count }, (_, index) => `Row ${index}`);

const List = ({ items }: { items: string[] }) => (
  <main>
    <VirtualRows
      items={items}
      keyOf={(item) => item}
      rowRem={3}
      aria-label="Rows"
      renderRow={(item, index, row) => (
        <li {...row} data-testid="row">
          {item} at {index}
        </li>
      )}
    />
  </main>
);

describe('Long lists', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('draws every row of a short list, as a plain list', () => {
    render(<List items={names(VIRTUAL_FROM)} />);
    const list = screen.getByRole('list', { name: 'Rows' });
    expect(within(list).getAllByTestId('row')).toHaveLength(VIRTUAL_FROM);
    expect(list.getAttribute('style')).toBeNull();
  });

  it('draws only the rows near the screen of a long one, keeping the room of the rest', () => {
    // jsdom has no layout: a phone-sized screen for the page's scroller.
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(400);
    render(<List items={names(5000)} />);
    const list = screen.getByRole('list', { name: 'Rows' });
    const rows = within(list).getAllByTestId('row');
    expect(rows.length).toBeGreaterThan(5);
    expect(rows.length).toBeLessThan(60);
    expect(rows[0]).toHaveTextContent('Row 0 at 0');
    expect(rows[0]).toHaveAttribute('data-index', '0');
    // The rows not drawn keep their room below, at the rows' 3 rem each.
    const below = Number.parseFloat(list.style.paddingBottom);
    expect(below).toBeGreaterThan((5000 - rows.length - 1) * 3 * 16 - 1);
    expect(list.style.paddingTop).toBe('0px');
  });
});
