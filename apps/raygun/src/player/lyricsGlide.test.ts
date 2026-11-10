import { createGlide } from './lyricsGlide';
import { followLine, lineAt } from './useFollowLine';

/** A scrolling box as the glide sees it: 400 px tall over 5000 px of lines. */
const makeBox = () => ({ scrollTop: 0, clientHeight: 400, scrollHeight: 5000 }) as HTMLElement;

describe('the lyrics glide', () => {
  let now = 0;
  let frames = new Map<number, FrameRequestCallback>();
  let nextId = 1;

  /** Runs the frames due, 1/60 s after the last. */
  const frame = (ms = 1000 / 60) => {
    now += ms;
    const due = [...frames.values()];
    frames = new Map();
    for (const callback of due) callback(now);
  };
  /** Frames until the glide stops, with the positions it passed through. */
  const settle = (box: HTMLElement, most = 120) => {
    const passed: number[] = [];
    for (let count = 0; count < most && frames.size > 0; count += 1) {
      frame();
      passed.push(box.scrollTop);
    }
    return passed;
  };

  beforeEach(() => {
    now = 1000;
    frames = new Map();
    vi.spyOn(performance, 'now').mockImplementation(() => now);
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      const id = nextId++;
      frames.set(id, callback);
      return id;
    });
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
      frames.delete(id);
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    delete document.documentElement.dataset.motion;
  });

  it('eases to the line and stops there, never past it', () => {
    const box = makeBox();
    createGlide(box).to(1000);
    expect(box.scrollTop).toBe(0);

    const passed = settle(box);
    expect(passed.at(-1)).toBe(1000);
    expect(frames.size).toBe(0);
    // Steady: every frame on from the last, never beyond the line.
    for (const [index, top] of passed.entries()) {
      expect(top).toBeGreaterThanOrEqual(passed[index - 1] ?? 0);
      expect(top).toBeLessThanOrEqual(1000);
    }
    // It eases in rather than jumping, is most of the way there in a third of a second and all
    // but there in 0.6 s, and then stops asking for frames.
    expect(passed[0]).toBeLessThan(100);
    expect(passed[19]).toBeGreaterThan(850);
    expect(passed[35]).toBeGreaterThan(985);
    expect(passed.length).toBeLessThan(90);
  });

  it('bends toward a newer line mid-glide instead of starting over', () => {
    const box = makeBox();
    const glide = createGlide(box);
    glide.to(1000);
    for (let count = 0; count < 8; count += 1) frame();
    const before = box.scrollTop;
    frame();
    const lastStep = box.scrollTop - before;
    expect(lastStep).toBeGreaterThan(0);

    glide.to(1400);
    const from = box.scrollTop;
    frame();
    // The move carries on at least as fast: it wasn't restarted from rest.
    expect(box.scrollTop - from).toBeGreaterThanOrEqual(lastStep);
    expect(settle(box).at(-1)).toBe(1400);
  });

  it('goes straight to the line where things may not move', () => {
    document.documentElement.dataset.motion = 'reduced';
    const box = makeBox();
    createGlide(box).to(1000);
    expect(box.scrollTop).toBe(1000);
    expect(frames.size).toBe(0);
  });

  it('keeps within what the box scrolls, and goes at once when asked', () => {
    const box = makeBox();
    const glide = createGlide(box);
    glide.to(1000);
    frame();
    glide.to(9000, true);
    expect(box.scrollTop).toBe(4600);
    expect(frames.size).toBe(0);
  });

  it('lets the listener take over', () => {
    const box = makeBox();
    createGlide(box).to(1000);
    frame();
    frame();
    // Scrolled by hand meanwhile.
    box.scrollTop = 50;
    frame();
    expect(box.scrollTop).toBe(50);
    expect(frames.size).toBe(0);
  });
});

describe('the line followed', () => {
  const STARTS = [0, 1000, 2000];

  it('is the last line started', () => {
    expect(lineAt(STARTS, -5)).toBe(-1);
    expect(lineAt(STARTS, 0)).toBe(0);
    expect(lineAt(STARTS, 1999)).toBe(1);
    expect(lineAt(STARTS, 2500)).toBe(2);
  });

  it("keeps its line when a report lands a moment before the line's start", () => {
    expect(followLine(STARTS, 1950, 2)).toBe(2);
    // A real step back, or a seek, moves it.
    expect(followLine(STARTS, 1500, 2)).toBe(1);
    expect(followLine(STARTS, 500, 2)).toBe(0);
    // Forward it always moves.
    expect(followLine(STARTS, 2000, 1)).toBe(2);
  });
});
