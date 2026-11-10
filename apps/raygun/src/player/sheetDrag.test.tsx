import type { PlaybackState } from '@raygun/bindings';
import { act, render, screen, within } from '@testing-library/react';
import type { FC } from 'react';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { IDLE_STATE } from '../test/bindingsMock';
import { SHEET_HOLD_LIMIT_MS, sheetDrag } from './swipe';
import {
  holdLiveValues,
  liveValuesHeld,
  markReceived,
  ticking,
  useLiveValue,
} from './useLivePosition';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const steps = () => {
  const release = vi.fn();
  const hold = vi.fn(() => release);
  return { hold, release, drag: sheetDrag(hold) };
};

describe('The full player’s drag', () => {
  it('holds the live values from its start until the sheet is back in place', () => {
    const { hold, release, drag } = steps();
    drag.start();
    drag.start();
    expect(hold).toHaveBeenCalledTimes(1);
    expect(drag.holding()).toBe(true);

    // Let go too soon: it snaps back, and only then do the values move again.
    expect(drag.end({ y: 40 }, { y: 100 })).toBe(false);
    expect(release).not.toHaveBeenCalled();
    drag.settled();
    expect(release).toHaveBeenCalledTimes(1);
    expect(drag.holding()).toBe(false);
  });

  it('keeps holding while a dismissed sheet slides away, until it has gone', () => {
    const { hold, release, drag } = steps();
    drag.start();
    expect(drag.end({ y: 200 }, { y: 0 })).toBe(true);
    drag.settled();
    drag.leaving();
    expect(hold).toHaveBeenCalledTimes(1);
    expect(release).not.toHaveBeenCalled();
    drag.gone();
    expect(release).toHaveBeenCalledTimes(1);
    // Gone twice (the exit, then the sheet taken down) lets go once.
    drag.gone();
    expect(release).toHaveBeenCalledTimes(1);
  });

  it('holds as the sheet slides away however it was closed, and lets go if it opens again', () => {
    const { hold, release, drag } = steps();
    drag.leaving();
    expect(hold).toHaveBeenCalledTimes(1);
    // Opened again on its way out: back in place, the values move again.
    drag.gone();
    expect(release).toHaveBeenCalledTimes(1);

    // The next drag that snaps back lets go as usual.
    drag.start();
    drag.end({ y: 10 }, { y: 0 });
    drag.settled();
    expect(release).toHaveBeenCalledTimes(2);
  });

  it('lets go by itself when nothing ends the drag', () => {
    vi.useFakeTimers();
    try {
      const { release, drag } = steps();
      drag.start();
      vi.advanceTimersByTime(SHEET_HOLD_LIMIT_MS - 1);
      expect(release).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(release).toHaveBeenCalledTimes(1);
      expect(drag.holding()).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('Held live values', () => {
  /** A view that follows the playhead, in whole seconds. */
  const Seconds: FC<{ state: PlaybackState }> = ({ state }) => (
    <span data-testid="seconds">{useLiveValue(state, (ms) => Math.floor(ms / 1000))}</span>
  );

  const report = (positionMs: number, status: 'playing' | 'paused' = 'playing') => {
    const state: PlaybackState = {
      ...IDLE_STATE,
      status,
      currentItemId: 'id-Intro',
      positionMs,
      durationMs: 180_000,
    };
    markReceived(state);
    return state;
  };

  it('stay where they were while held, reports included, and catch up when let go', () => {
    const first = report(10_000);
    const view = render(<Seconds state={first} />);
    expect(screen.getByTestId('seconds')).toHaveTextContent('10');
    expect(ticking()).toBe(true);

    const release = holdLiveValues(first);
    expect(liveValuesHeld()).toBe(true);
    // No timer while held, and a report changes nothing on screen.
    expect(ticking()).toBe(false);
    view.rerender(<Seconds state={report(40_000)} />);
    expect(screen.getByTestId('seconds')).toHaveTextContent('10');

    act(() => release());
    expect(liveValuesHeld()).toBe(false);
    expect(screen.getByTestId('seconds')).toHaveTextContent('40');
    expect(ticking()).toBe(true);
    // Letting go twice does nothing more.
    act(() => release());
    expect(screen.getByTestId('seconds')).toHaveTextContent('40');
    view.unmount();
  });

  it('let a paused view catch up too', () => {
    const paused = report(5_000, 'paused');
    const view = render(<Seconds state={paused} />);
    const release = holdLiveValues(paused);
    view.rerender(<Seconds state={report(7_000, 'paused')} />);
    expect(screen.getByTestId('seconds')).toHaveTextContent('5');
    act(() => release());
    expect(screen.getByTestId('seconds')).toHaveTextContent('7');
    view.unmount();
  });
});

describe('The full player', () => {
  it('is a layer of its own, over a wash of the cover drawn once', async () => {
    AppShellWrapper.seedCore();
    // jsdom draws no canvas: the wash asks for one and draws nothing.
    const canvas = vi
      .spyOn(HTMLCanvasElement.prototype, 'getContext')
      .mockImplementation(() => null);
    try {
      await AppShellWrapper.mount();
      AppShellWrapper.playing(
        [{ ...makeItem('Intro', 'Raygun'), artworkUrl: 'https://img.example/cover.jpg' }],
        0,
      );
      await AppShellWrapper.miniPlayer.open();
      const sheet = await AppShellWrapper.playerSheet.element;
      expect(sheet).toHaveClass('will-change-transform');
      expect(within(sheet).getByTestId('cover-wash')).toBeInTheDocument();
      expect(canvas).toHaveBeenCalledWith('2d');
      // No full-screen picture left to blur on every frame.
      expect(sheet.querySelector('img[class*="blur"]')).toBeNull();
    } finally {
      canvas.mockRestore();
    }
  });
});
