import { fireEvent, render, screen } from '@testing-library/react';

import { SkeletonSwap } from './SkeletonSwap';

const Row = ({ ready }: { ready: boolean }) => (
  <SkeletonSwap ready={ready} skeleton={<p>Placeholders</p>}>
    <p>Cards</p>
  </SkeletonSwap>
);

describe('a skeleton swapped for what it stood for', () => {
  afterEach(() => {
    delete document.documentElement.dataset.motion;
  });

  it('fades the skeleton out over the cards that came, then lets it go', () => {
    document.documentElement.dataset.motion = 'lively';
    const { rerender } = render(<Row ready={false} />);
    expect(screen.getByText('Placeholders')).toBeInTheDocument();
    expect(screen.queryByText('Cards')).not.toBeInTheDocument();

    rerender(<Row ready />);
    expect(screen.getByText('Cards')).toBeInTheDocument();
    const leaving = screen.getByTestId('skeleton-leaving');
    expect(leaving).toHaveAttribute('aria-hidden', 'true');
    expect(leaving).toHaveTextContent('Placeholders');

    fireEvent.transitionEnd(leaving);
    expect(screen.queryByTestId('skeleton-leaving')).not.toBeInTheDocument();
    expect(screen.queryByText('Placeholders')).not.toBeInTheDocument();
  });

  it('shows what is there from the start at once, and swaps at once where nothing may move', () => {
    document.documentElement.dataset.motion = 'lively';
    render(<Row ready />);
    expect(screen.queryByTestId('skeleton-leaving')).not.toBeInTheDocument();

    document.documentElement.dataset.motion = 'reduced';
    const { rerender } = render(<Row ready={false} />);
    rerender(<Row ready />);
    expect(screen.queryByTestId('skeleton-leaving')).not.toBeInTheDocument();
  });
});
