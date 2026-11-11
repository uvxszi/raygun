import { act, render, screen } from '@testing-library/react';

import { SkipNotice } from './SkipNotice';

const state = vi.hoisted(() => ({
  player: { error: 'Offline mode is on', skippedItemId: 'song' } as Record<string, unknown>,
}));

vi.mock('./usePlayback', () => ({
  usePlayerSelect: (pick: (value: unknown) => unknown) => pick(state.player),
  useQueueSelect: (pick: (value: unknown) => unknown) => pick(null),
}));

describe('the skip notice', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('stays gone once shown, when the player bar is drawn again', () => {
    vi.useFakeTimers();
    const first = render(<SkipNotice />);
    expect(screen.getByTestId('skip-notice')).toHaveTextContent('Offline mode is on');
    act(() => {
      vi.advanceTimersByTime(8_000);
    });
    expect(screen.queryByTestId('skip-notice')).not.toBeInTheDocument();
    first.unmount();

    // A rotation or another page draws the bar again while the state still names the skip.
    render(<SkipNotice />);
    expect(screen.queryByTestId('skip-notice')).not.toBeInTheDocument();
  });
});
