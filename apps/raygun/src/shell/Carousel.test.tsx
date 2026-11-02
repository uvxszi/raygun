import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import '../i18n';
import { Carousel } from './Carousel';

/** jsdom has no layout: give the row a width to page through. */
const sized = (row: HTMLElement, scrollLeft: number) => {
  Object.defineProperties(row, {
    scrollWidth: { configurable: true, value: 1200 },
    clientWidth: { configurable: true, value: 400 },
    scrollLeft: { configurable: true, writable: true, value: scrollLeft },
  });
  const scrollBy = vi.fn();
  row.scrollBy = scrollBy as unknown as typeof row.scrollBy;
  return scrollBy;
};

describe('Carousels', () => {
  it('page with their side buttons, which show only where there is more', async () => {
    const user = userEvent.setup();
    const { container } = render(
      <Carousel testId="row" className="gap-3">
        <li>One</li>
        <li>Two</li>
      </Carousel>,
    );
    const row = screen.getByTestId('row');
    // Nothing to scroll: no buttons.
    expect(within(container).queryByRole('button')).toBeNull();

    const scrollBy = sized(row, 0);
    row.dispatchEvent(new Event('scroll'));
    const forward = await screen.findByRole('button', { name: 'Scroll forward' });
    expect(screen.queryByRole('button', { name: 'Scroll back' })).toBeNull();
    await user.click(forward);
    expect(scrollBy).toHaveBeenCalledWith({ left: 340, behavior: 'smooth' });

    sized(row, 800);
    row.dispatchEvent(new Event('scroll'));
    expect(await screen.findByRole('button', { name: 'Scroll back' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Scroll forward' })).toBeNull();
    expect(row.className).toContain('no-scrollbar');
  });
});
