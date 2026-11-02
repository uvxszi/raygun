import { fireEvent, render, screen } from '@testing-library/react';
import type { FC } from 'react';

import { useHorizontalWheel } from './useHorizontalWheel';

const Row: FC = () => {
  useHorizontalWheel();
  return (
    <div data-testid="row" style={{ overflowX: 'auto' }}>
      <span data-testid="card">card</span>
    </div>
  );
};

/** jsdom has no layout: give the row a width to scroll through. */
const sized = (row: HTMLElement, scrollLeft: number) => {
  Object.defineProperties(row, {
    scrollWidth: { configurable: true, value: 1000 },
    clientWidth: { configurable: true, value: 400 },
    scrollLeft: { configurable: true, writable: true, value: scrollLeft },
  });
  const scrollBy = vi.fn();
  row.scrollBy = scrollBy as unknown as typeof row.scrollBy;
  return scrollBy;
};

describe('Sideways rows', () => {
  it('scroll with the mouse wheel, until their end', () => {
    render(<Row />);
    const row = screen.getByTestId('row');

    const scrollBy = sized(row, 0);
    const turned = !fireEvent.wheel(screen.getByTestId('card'), { deltaY: 120 });
    expect(scrollBy).toHaveBeenCalledWith({ left: 120, behavior: 'auto' });
    expect(turned).toBe(true);

    // At the end the page scrolls instead, and Ctrl keeps zooming.
    const atEnd = sized(row, 600);
    expect(fireEvent.wheel(screen.getByTestId('card'), { deltaY: 120 })).toBe(true);
    fireEvent.wheel(screen.getByTestId('card'), { deltaY: -120, ctrlKey: true });
    expect(atEnd).not.toHaveBeenCalled();
  });
});
