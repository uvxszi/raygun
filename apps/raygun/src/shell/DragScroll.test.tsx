import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import '../i18n';
import { Carousel } from './Carousel';
import { DRAG_THRESHOLD_PX, releaseSpeed, swipeScrollsRow, useDragScroll } from './useDragScroll';

const NAMES = ['One', 'Two', 'Three'];

const Row: FC<{ onPick: (name: string) => void }> = ({ onPick }) => {
  const row = useDragScroll<HTMLDivElement>();
  return (
    <div ref={row} data-testid="row">
      {NAMES.map((name) => (
        <button key={name} type="button" onClick={() => onPick(name)}>
          {name}
        </button>
      ))}
    </div>
  );
};

/** jsdom has no layout: give the row a width, and more or less to show than fits. */
const sized = (row: HTMLElement, scrollWidth = 1000, scrollLeft = 100) => {
  Object.defineProperties(row, {
    scrollWidth: { configurable: true, value: scrollWidth },
    clientWidth: { configurable: true, value: 400 },
    scrollLeft: { configurable: true, writable: true, value: scrollLeft },
  });
};

const setup = () => {
  const onPick = vi.fn();
  render(<Row onPick={onPick} />);
  const row = screen.getByTestId('row');
  sized(row);
  return { onPick, row, two: screen.getByRole('button', { name: 'Two' }) };
};

describe('Rows dragged with a mouse', () => {
  beforeEach(() => {
    // No glide unless a test asks for one.
    document.documentElement.dataset.motion = 'off';
  });
  afterEach(() => {
    delete document.documentElement.dataset.motion;
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it(`stay clicks under ${DRAG_THRESHOLD_PX} px; past it the row follows and nothing is clicked`, async () => {
    const user = userEvent.setup();
    const { onPick, row, two } = setup();

    // A press that wobbles a little is still a click.
    await user.pointer([
      { keys: '[MouseLeft>]', target: two, coords: { clientX: 200, clientY: 10 } },
      { coords: { clientX: 204, clientY: 16 } },
      { keys: '[/MouseLeft]' },
    ]);
    expect(onPick).toHaveBeenCalledWith('Two');
    expect(row.scrollLeft).toBe(100);
    onPick.mockClear();

    await user.pointer([
      { keys: '[MouseLeft>]', target: two, coords: { clientX: 200 } },
      { coords: { clientX: 190 } },
      { coords: { clientX: 150 } },
      { keys: '[/MouseLeft]' },
    ]);
    expect(row.scrollLeft).toBe(150);
    expect(onPick).not.toHaveBeenCalled();

    // The drag's click only: the next one is a click again.
    await user.click(two);
    expect(onPick).toHaveBeenCalledWith('Two');
  });

  it('show a closed hand while dragged, and keep the browser from dragging what is under it', async () => {
    const user = userEvent.setup();
    const { row, two } = setup();
    const root = document.documentElement;

    await user.hover(two);
    expect(row).toHaveAttribute('data-drag-scroll', 'more');
    await user.pointer([
      { keys: '[MouseLeft>]', target: two, coords: { clientX: 200 } },
      { coords: { clientX: 260 } },
    ]);
    expect(row.scrollLeft).toBe(40);
    expect(root).toHaveAttribute('data-drag-scrolling');
    expect(fireEvent.dragStart(two)).toBe(false);

    await user.pointer({ keys: '[/MouseLeft]' });
    expect(root).not.toHaveAttribute('data-drag-scrolling');
    expect(fireEvent.dragStart(two)).toBe(true);
  });

  it('leave fingers, other buttons and rows that fit to the browser', async () => {
    const user = userEvent.setup();
    const { onPick, row, two } = setup();

    await user.pointer([
      { keys: '[TouchA>]', target: two, coords: { clientX: 200 } },
      { pointerName: 'TouchA', coords: { clientX: 120 } },
      { keys: '[/TouchA]' },
    ]);
    expect(row.scrollLeft).toBe(100);

    await user.pointer([
      { keys: '[MouseRight>]', target: two, coords: { clientX: 200 } },
      { coords: { clientX: 120 } },
      { keys: '[/MouseRight]' },
    ]);
    expect(row.scrollLeft).toBe(100);

    // Everything shows: no hand, no drag, the click stays.
    sized(row, 400, 0);
    await user.unhover(two);
    await user.hover(two);
    expect(row).toHaveAttribute('data-drag-scroll', '');
    onPick.mockClear();
    await user.pointer([
      { keys: '[MouseLeft>]', target: two, coords: { clientX: 200 } },
      { coords: { clientX: 120 } },
      { keys: '[/MouseLeft]' },
    ]);
    expect(row.scrollLeft).toBe(0);
    expect(onPick).toHaveBeenCalledWith('Two');
  });

  /** Frames run by hand, and a drag let go of while still moving: 60 px in 48 ms. */
  const fling = async () => {
    document.documentElement.dataset.motion = 'lively';
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    });
    const cancel = vi.fn();
    vi.stubGlobal('cancelAnimationFrame', cancel);
    // jsdom stamps events with Date.now().
    let now = 1000;
    vi.spyOn(Date, 'now').mockImplementation(() => now);
    const user = userEvent.setup();
    const shown = setup();

    await user.pointer({ keys: '[MouseLeft>]', target: shown.two, coords: { clientX: 300 } });
    for (const clientX of [280, 260, 240]) {
      now += 16;
      await user.pointer({ coords: { clientX } });
    }
    now += 4;
    await user.pointer({ keys: '[/MouseLeft]' });
    return { ...shown, user, frames, cancel };
  };

  it('glide on when let go while moving, and a press on the gliding row only stops it', async () => {
    const { onPick, row, two, user, frames, cancel } = await fling();
    expect(row.scrollLeft).toBe(160);
    expect(onPick).not.toHaveBeenCalled();

    // 40 px in 32 ms: it carries on the same way, slowing.
    frames.shift()?.(0);
    expect(row.scrollLeft).toBeCloseTo(180);
    frames.shift()?.(16);
    expect(row.scrollLeft).toBeGreaterThan(195);
    expect(row.scrollLeft).toBeLessThan(200);

    await user.click(two);
    expect(cancel).toHaveBeenCalled();
    expect(onPick).not.toHaveBeenCalled();
    await user.click(two);
    expect(onPick).toHaveBeenCalledWith('Two');
  });

  it('stop gliding when something else scrolls the row', async () => {
    const { row, frames } = await fling();
    frames.shift()?.(0);
    // The row's page button, say.
    row.scrollLeft = 500;
    frames.shift()?.(16);
    expect(row.scrollLeft).toBe(500);
    expect(frames).toHaveLength(0);
  });

  it('carousels drag too, without opening the card under the mouse', async () => {
    const user = userEvent.setup();
    const onOpen = vi.fn();
    render(
      <Carousel testId="carousel">
        {NAMES.map((name) => (
          <li key={name}>
            <button type="button" onClick={() => onOpen(name)}>
              {name}
            </button>
          </li>
        ))}
      </Carousel>,
    );
    const row = screen.getByTestId('carousel');
    sized(row, 1200, 0);
    await user.pointer([
      { keys: '[MouseLeft>]', target: screen.getByText('Two'), coords: { clientX: 300 } },
      { coords: { clientX: 100 } },
      { keys: '[/MouseLeft]' },
    ]);
    expect(row.scrollLeft).toBe(200);
    expect(onOpen).not.toHaveBeenCalled();
    // The row's own snapping is back once the drag is over.
    expect(row.style.scrollSnapType).toBe('');
  });
});

describe('The speed a drag is let go at', () => {
  it('comes from the last moves, unless the mouse had stopped', () => {
    const moves = [
      { x: 300, at: 0 },
      { x: 280, at: 16 },
      { x: 260, at: 32 },
    ];
    expect(releaseSpeed(moves, 36)).toBeCloseTo(-1.25);
    expect(releaseSpeed(moves, 100)).toBe(0);
    expect(releaseSpeed(moves.slice(0, 1), 4)).toBe(0);
    // Older moves don't count, and a jump is capped.
    expect(
      releaseSpeed(
        [
          { x: 0, at: 0 },
          { x: 100, at: 150 },
          { x: 110, at: 160 },
        ],
        165,
      ),
    ).toBeCloseTo(1);
    expect(
      releaseSpeed(
        [
          { x: 0, at: 0 },
          { x: 1000, at: 1 },
        ],
        2,
      ),
    ).toBe(5);
  });
});

describe('A swipe from the right edge', () => {
  it('scrolls a row that has more to show, and is the edge pull once the row is at its end', () => {
    const { row, two } = setup();
    expect(swipeScrollsRow(two)).toBe(true);
    sized(row, 1000, 600);
    expect(swipeScrollsRow(two)).toBe(false);
    expect(swipeScrollsRow(document.body)).toBe(false);
    expect(swipeScrollsRow(window)).toBe(false);
  });
});
