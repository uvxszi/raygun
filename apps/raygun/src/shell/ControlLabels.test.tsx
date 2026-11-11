import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { useCoverSheetStore } from '../music/coverSheetStore';
import { useCoverHold } from '../music/useCoverHold';
import { useHoldPress } from '../music/useHoldPress';
import { ControlLabels, HOVER_MS, LABEL_HOLD_MS, LINGER_MS } from './ControlLabels';
import { keepOnScreen, labelFor, placeLabel } from './labelRules';

const NO_INSETS = { top: 0, right: 0, bottom: 0, left: 0 };

const MoveButton: FC<{ onTap: () => void; onHold: () => void }> = ({ onTap, onHold }) => {
  const { props } = useHoldPress({ onTap, onHold });
  return (
    <button type="button" aria-label="Move up" {...props}>
      <svg aria-hidden />
    </button>
  );
};

const CoverButton: FC = () => {
  const { props } = useCoverHold({
    cover: { url: 'https://example.com/cover.jpg', title: 'Intro' },
  });
  return (
    <button type="button" aria-label="Open cover" {...props}>
      <img src="https://example.com/cover.jpg" alt="" />
    </button>
  );
};

type Handlers = {
  shuffle: () => void;
  playAll: () => void;
  cast: () => void;
  moveTap: () => void;
  moveHold: () => void;
};

const renderPage = () => {
  const handlers: Handlers = {
    shuffle: vi.fn(),
    playAll: vi.fn(),
    cast: vi.fn(),
    moveTap: vi.fn(),
    moveHold: vi.fn(),
  };
  render(
    <>
      <button type="button" aria-label="Shuffle" onClick={handlers.shuffle}>
        <svg aria-hidden />
      </button>
      <button type="button" aria-label="Play all songs" onClick={handlers.playAll}>
        Play all
      </button>
      <button type="button" title="Cast to a TV" onClick={handlers.cast}>
        <svg aria-hidden />
      </button>
      <MoveButton onTap={handlers.moveTap} onHold={handlers.moveHold} />
      <CoverButton />
      <p>Somewhere else</p>
      <ControlLabels />
    </>,
  );
  return handlers;
};

const finger = { pointerId: 3, pointerType: 'touch', button: 0, clientX: 40, clientY: 300 };

/** A finger down on `element`, held `ms`, then lifted (the webview's click after it too). */
const hold = (element: Element, ms: number, at: Partial<typeof finger> = {}) => {
  fireEvent.pointerDown(element, { ...finger, ...at });
  act(() => vi.advanceTimersByTime(ms));
};
const lift = (element: Element, at: Partial<typeof finger> = {}) => {
  fireEvent.pointerUp(element, { ...finger, ...at });
  fireEvent.click(element);
};

describe('Names of icon buttons', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('shows a held icon button’s name, and lifting the finger then doesn’t press it', () => {
    const handlers = renderPage();
    const shuffle = screen.getByRole('button', { name: 'Shuffle' });

    hold(shuffle, LABEL_HOLD_MS);
    expect(screen.getByRole('tooltip')).toHaveTextContent('Shuffle');
    lift(shuffle);
    expect(handlers.shuffle).not.toHaveBeenCalled();
    // It stays a moment for reading, then goes.
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
    act(() => vi.advanceTimersByTime(LINGER_MS));
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();

    // A tap still presses it, without a name.
    hold(shuffle, 120);
    lift(shuffle);
    expect(handlers.shuffle).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('shows no name for a finger that moves on (a scroll or a drag)', () => {
    renderPage();
    const shuffle = screen.getByRole('button', { name: 'Shuffle' });
    fireEvent.pointerDown(shuffle, finger);
    fireEvent.pointerMove(shuffle, { ...finger, clientY: finger.clientY - 30 });
    act(() => vi.advanceTimersByTime(LABEL_HOLD_MS * 2));
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });

  it('leaves buttons that say what they do, and holds of their own, as they were', () => {
    const handlers = renderPage();
    const playAll = screen.getByRole('button', { name: 'Play all songs' });
    hold(playAll, LABEL_HOLD_MS * 2);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    lift(playAll);
    expect(handlers.playAll).toHaveBeenCalledTimes(1);

    // Holding a move button moves the song to the top, as before: no name.
    const move = screen.getByRole('button', { name: 'Move up' });
    hold(move, 600);
    expect(handlers.moveHold).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    lift(move);
    expect(handlers.moveTap).not.toHaveBeenCalled();

    // Holding a cover opens its save options, as before: no name.
    hold(screen.getByRole('button', { name: 'Open cover' }), 600);
    expect(useCoverSheetStore.getState().cover?.title).toBe('Intro');
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    act(() => useCoverSheetStore.getState().close());
  });

  it('says it at the bottom when there’s no room by the button', () => {
    renderPage();
    const shuffle = screen.getByRole('button', { name: 'Shuffle' });
    // Pressed at the very bottom of the screen, with the button at the top (jsdom's boxes are
    // empty): neither above nor below the finger fits.
    hold(shuffle, LABEL_HOLD_MS, { clientY: window.innerHeight - 4 });
    expect(screen.getByRole('tooltip')).toHaveAttribute('data-place', 'toast');
  });

  it('names a button the mouse rests on instead of the browser’s tip, never for a finger', async () => {
    vi.useRealTimers();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    renderPage();
    const cast = screen.getByRole('button', { name: 'Cast to a TV' });

    await user.hover(cast);
    // The browser's own tip doesn't show over the label; the name stays for screen readers.
    expect(cast).not.toHaveAttribute('title');
    expect(screen.getByRole('button', { name: 'Cast to a TV' })).toBe(cast);
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(HOVER_MS));
    expect(screen.getByRole('tooltip')).toHaveTextContent('Cast to a TV');
    expect(screen.getByRole('tooltip')).toHaveAttribute('data-place', 'bubble');

    await user.hover(screen.getByText('Somewhere else'));
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
    expect(cast).toHaveAttribute('title', 'Cast to a TV');

    // A finger passing over shows nothing.
    fireEvent.pointerOver(screen.getByRole('button', { name: 'Shuffle' }), finger);
    act(() => vi.advanceTimersByTime(HOVER_MS * 2));
    expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
  });
});

describe('Label rules', () => {
  const button = (html: string) => {
    const host = document.createElement('div');
    host.innerHTML = html;
    return host.firstElementChild as HTMLElement;
  };

  it('names icons, numbers and marks, never controls that spell out what they do', () => {
    expect(labelFor(button('<button aria-label="Shuffle"><svg></svg></button>'), 'touch')).toBe(
      'Shuffle',
    );
    expect(labelFor(button('<a title="Downloads"><svg></svg>3</a>'), 'touch')).toBe('Downloads');
    expect(labelFor(button('<button aria-label="Speed">1.5×</button>'), 'touch')).toBe('Speed');
    // Words on it: a finger's hold leaves it alone, the mouse still names it.
    const row = button('<button aria-label="Play Intro by Raygun">Intro</button>');
    expect(labelFor(row, 'touch')).toBeNull();
    expect(labelFor(row, 'pointer')).toBe('Play Intro by Raygun');
    // Its name is what it says: nothing to add.
    expect(labelFor(button('<button aria-label="Save">Save</button>'), 'pointer')).toBeNull();
    // Words kept for screen readers aren't shown.
    expect(
      labelFor(
        button('<a aria-label="Dune"><img alt=""><span class="sr-only">Dune</span></a>'),
        'touch',
      ),
    ).toBe('Dune');
    expect(labelFor(button('<button><svg></svg></button>'), 'touch')).toBeNull();
  });

  it('places a finger’s label above the button, else below the finger, else as a toast', () => {
    const screenBox = { width: 400, height: 800, safe: { ...NO_INSETS, top: 24, bottom: 48 } };
    const middle = { left: 100, top: 400, right: 140, bottom: 440 };
    expect(placeLabel(middle, screenBox, 'touch', 420)).toEqual({
      kind: 'bubble',
      x: 120,
      y: 392,
      above: true,
    });
    // Under the status bar: below the finger instead.
    const top = { left: 340, top: 30, right: 380, bottom: 70 };
    expect(placeLabel(top, screenBox, 'touch', 50)).toMatchObject({ kind: 'bubble', above: false });
    expect(placeLabel(top, screenBox, 'touch', 50)).toMatchObject({ y: 86 });
    // A tall control filling the screen: a toast.
    const tall = { left: 0, top: 30, right: 400, bottom: 750 };
    expect(placeLabel(tall, screenBox, 'touch', 740)).toEqual({ kind: 'toast' });
    // The mouse's goes below, else above.
    expect(placeLabel(middle, screenBox, 'pointer')).toMatchObject({ above: false, y: 448 });
    const low = { left: 100, top: 700, right: 140, bottom: 740 };
    expect(placeLabel(low, screenBox, 'pointer')).toMatchObject({ above: true, y: 692 });
  });

  it('keeps a bubble inside the safe area', () => {
    const screenBox = { width: 400, height: 800, safe: { ...NO_INSETS, left: 30, right: 44 } };
    expect(keepOnScreen(10, 90, screenBox)).toBe(28);
    expect(keepOnScreen(300, 380, screenBox)).toBe(-32);
    expect(keepOnScreen(100, 200, screenBox)).toBe(0);
  });
});
