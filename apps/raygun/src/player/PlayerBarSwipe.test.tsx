import type { QueueSnapshot } from '@raygun/bindings';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { MotionGlobalConfig } from 'motion/react';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { comeIn, swipeNeighbor, throwDuration, velocityTracker } from './swipe';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second'), makeItem('Third')];

const setPhone = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

/** A finger on the bar's song, moved by (dx, dy) in a few steps, then lifted. */
const swipe = (dx: number, dy: number) => {
  const bar = AppShellWrapper.miniPlayer.element;
  const song = within(bar).getByRole('button', { name: 'Open player' });
  fireEvent.pointerDown(song, { pointerId: 1, clientX: 200, clientY: 300 });
  for (const step of [0.25, 0.5, 0.75, 1]) {
    fireEvent.pointerMove(bar, {
      pointerId: 1,
      clientX: 200 + dx * step,
      clientY: 300 + dy * step,
    });
  }
  fireEvent.pointerUp(bar, { pointerId: 1, clientX: 200 + dx, clientY: 300 + dy });
};

const title = () => screen.getByTestId('mini-player-title');

/** Where (px, sideways) the bar draws the song now. */
const placeOf = (element: HTMLElement) => {
  const match = /translateX\((-?[\d.]+)px\)/.exec(element.style.transform);
  return match ? Number(match[1]) : 0;
};

/** Every place the bar draws the song at, frame by frame, until `stop`. */
const followPlaces = (element: HTMLElement) => {
  const places: number[] = [];
  const observer = new MutationObserver(() => places.push(placeOf(element)));
  observer.observe(element, { attributes: true, attributeFilter: ['style'] });
  return {
    stop: () => {
      observer.disconnect();
      return places;
    },
  };
};

describe('Swiping the phone player bar', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    setPhone();
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
    delete document.documentElement.dataset.motion;
  });

  it('throws the song out and brings the next one in from the queue at once', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 1);
    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Second');

    swipe(-160, 4);
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'next' });
    // The player hasn't reported the next song: the bar shows it from the queue all the same.
    await waitFor(() => expect(title()).toHaveTextContent('Third'));

    // A swipe right brings the one before back.
    swipe(160, -3);
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'playItem',
      itemId: 'id-Intro',
    });
  });

  it('brings the same song back when there is nothing after or before it', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 2);
    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Third');

    swipe(-160, 0);
    // Next still asks the player (at the end without repeat it does nothing).
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'next' });
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(title()).toHaveTextContent('Third');

    AppShellWrapper.playing(QUEUE, 0);
    await waitFor(() => expect(title()).toHaveTextContent('Intro'));
    swipe(160, 0);
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'seek', positionMs: 0 });
    await new Promise((resolve) => setTimeout(resolve, 400));
    expect(title()).toHaveTextContent('Intro');
  });

  it('swaps at once with motion off, and swiping up still opens the full player', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Intro');
    // As the shell sets it from Settings → Appearance (or the system).
    document.documentElement.dataset.motion = 'off';

    swipe(-160, 0);
    expect(title()).toHaveTextContent('Second');

    commands.playbackControl.mockClear();
    swipe(3, -120);
    expect(await AppShellWrapper.playerSheet.element).toBeInTheDocument();
    expect(commands.playbackControl).not.toHaveBeenCalledWith({ type: 'next' });
  });

  it('brings the next song in from the right and the one before from the left', async () => {
    // The springs run for real here: their way in is what's checked.
    MotionGlobalConfig.skipAnimations = false;
    try {
      await AppShellWrapper.mount();
      AppShellWrapper.playing(QUEUE, 1);
      expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Second');
      const song = within(AppShellWrapper.miniPlayer.element).getByRole('button', {
        name: 'Open player',
      });

      const left = followPlaces(song);
      swipe(-160, 0);
      await waitFor(() => expect(title()).toHaveTextContent('Third'));
      await waitFor(() => expect(placeOf(song)).toBe(0), { timeout: 3000 });
      const thrownLeft = left.stop();
      const cameIn = thrownLeft.findIndex((place) => place > 0);
      // Out to the left first, then in from the right, never past its place to the left.
      expect(cameIn).toBeGreaterThan(0);
      expect(Math.min(...thrownLeft.slice(0, cameIn))).toBeLessThan(-300);
      expect(Math.max(...thrownLeft.slice(cameIn))).toBeLessThanOrEqual(400);
      expect(Math.min(...thrownLeft.slice(cameIn))).toBeGreaterThan(-40);

      AppShellWrapper.playing(QUEUE, 2);
      await waitFor(() => expect(title()).toHaveTextContent('Third'));
      const right = followPlaces(song);
      swipe(160, 0);
      await waitFor(() => expect(title()).toHaveTextContent('Second'));
      await waitFor(() => expect(placeOf(song)).toBe(0), { timeout: 3000 });
      const thrownRight = right.stop();
      const cameBack = thrownRight.findIndex((place) => place < 0);
      expect(cameBack).toBeGreaterThan(0);
      expect(Math.max(...thrownRight.slice(0, cameBack))).toBeGreaterThan(300);
      expect(Math.min(...thrownRight.slice(cameBack))).toBeGreaterThanOrEqual(-400);
      expect(Math.max(...thrownRight.slice(cameBack))).toBeLessThan(40);
    } finally {
      MotionGlobalConfig.skipAnimations = true;
    }
  });

  it('a tap without moving still opens the full player', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    const bar = await AppShellWrapper.miniPlayer.title;
    fireEvent.pointerDown(bar, { pointerId: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(bar, { pointerId: 1, clientX: 11, clientY: 10 });
    fireEvent.click(bar);
    expect(await AppShellWrapper.playerSheet.element).toBeInTheDocument();
  });
});

describe('Swipe helpers', () => {
  const queue = (order: number[]): QueueSnapshot =>
    ({ items: QUEUE, order }) as unknown as QueueSnapshot;

  it('finds the song a swipe brings in, in play order', () => {
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Second', false, 'next')?.title).toBe('Third');
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Second', false, 'previous')?.title).toBe('Intro');
    // Shuffled: the order, not the list.
    expect(swipeNeighbor(queue([2, 0, 1]), 'id-Third', false, 'next')?.title).toBe('Intro');
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Third', false, 'next')).toBeNull();
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Third', true, 'next')?.title).toBe('Intro');
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Intro', false, 'previous')).toBeNull();
    expect(swipeNeighbor(queue([0, 1, 2]), 'id-Intro', true, 'previous')?.title).toBe('Third');
    expect(swipeNeighbor(queue([0]), 'id-Intro', true, 'next')).toBeNull();
  });

  it('measures a let-go’s speed over its last moves, and throws at least that fast', () => {
    const tracker = velocityTracker();
    tracker.add(0, 0, 0);
    tracker.add(0, 0, 500);
    tracker.add(50, 0, 550);
    tracker.add(100, 10, 600);
    expect(tracker.velocity()).toEqual({ x: 1000, y: 100 });
    expect(throwDuration(300, 3000)).toBeCloseTo(0.1);
    expect(throwDuration(300, 100)).toBeCloseTo(300 / 1800);
    expect(throwDuration(2000, 100)).toBe(0.24);
  });

  it('brings a song in from the side opposite the throw, going the throw’s way', () => {
    // Thrown left (next): in from the right, moving left.
    expect(comeIn('next', -2500, 360)).toEqual({ from: 360, velocity: -2500 });
    // Thrown right (the one before): in from the left, moving right.
    expect(comeIn('previous', 2500, 360)).toEqual({ from: -360, velocity: 2500 });
    // At least the throw's own pace, at most what the spring settles from without overshooting.
    expect(comeIn('next', 0, 360).velocity).toBe(-1800);
    expect(comeIn('previous', 90_000, 360).velocity).toBe(6000);
  });
});
