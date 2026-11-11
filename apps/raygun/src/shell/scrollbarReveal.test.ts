import { FADE_MS, SHOWN_MS, startScrollbarReveal } from './scrollbarReveal';

/** A scroller with a bar 10 px wide at its right (jsdom lays nothing out). */
const scroller = () => {
  const element = document.createElement('div');
  const sizes = {
    clientWidth: 100,
    clientHeight: 100,
    scrollWidth: 100,
    scrollHeight: 500,
    clientLeft: 0,
    clientTop: 0,
  };
  for (const [name, value] of Object.entries(sizes)) {
    Object.defineProperty(element, name, { value, configurable: true });
  }
  element.getBoundingClientRect = () =>
    ({ x: 0, y: 0, left: 0, top: 0, right: 110, bottom: 100, width: 110, height: 100 }) as DOMRect;
  const inside = document.createElement('p');
  element.append(inside);
  document.body.append(element);
  return { element, inside };
};

const scrolled = (element: Element) => element.dispatchEvent(new Event('scroll'));

const pointer = (type: string, target: Element, clientX: number, pointerType = 'mouse') =>
  target.dispatchEvent(
    Object.assign(new Event(type, { bubbles: true }), { pointerType, clientX, clientY: 50 }),
  );

const state = (element: Element) => element.getAttribute('data-scrollbar');

describe('Scrollbars shown while scrolling', () => {
  let stop: () => void;

  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance', 'Date'] });
    stop = startScrollbarReveal();
  });

  afterEach(() => {
    stop();
    document.body.replaceChildren();
    vi.useRealTimers();
  });

  it('shows a bar scrolled with the wheel, then fades it out a second later', () => {
    const { element, inside } = scroller();
    inside.dispatchEvent(new WheelEvent('wheel', { bubbles: true, deltaY: 100 }));
    scrolled(element);
    expect(state(element)).toBe('shown');

    // Smooth scrolling carries on without more wheel: still shown.
    for (let frame = 0; frame < 20; frame += 1) {
      vi.advanceTimersByTime(16);
      scrolled(element);
    }
    vi.advanceTimersByTime(SHOWN_MS - 1);
    expect(state(element)).toBe('shown');
    vi.advanceTimersByTime(1);
    expect(state(element)).toBe('fading');
    vi.advanceTimersByTime(FADE_MS);
    expect(state(element)).toBeNull();
  });

  it("shows nothing for the app's own scrolls", () => {
    const { element, inside } = scroller();
    const lyrics = scroller();
    // The lyrics glide on their own: no input at all.
    scrolled(lyrics.element);
    expect(state(lyrics.element)).toBeNull();

    // The wheel turns over another list while they glide: only that list shows its bar.
    inside.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    scrolled(lyrics.element);
    scrolled(element);
    expect(state(lyrics.element)).toBeNull();
    expect(state(element)).toBe('shown');

    // A row brought into view a while after the last scroll by hand.
    vi.advanceTimersByTime(SHOWN_MS + FADE_MS);
    scrolled(element);
    expect(state(element)).toBeNull();
  });

  it('follows a finger, but not one dragging something along a list', () => {
    const { element, inside } = scroller();
    inside.dispatchEvent(new Event('touchmove', { bubbles: true, cancelable: true }));
    scrolled(element);
    expect(state(element)).toBe('shown');
    vi.advanceTimersByTime(SHOWN_MS + FADE_MS);

    // A row dragged: the drag takes the finger's moves, and the list scrolls under it by itself.
    inside.addEventListener('touchmove', (event) => event.preventDefault(), { passive: false });
    inside.dispatchEvent(new Event('touchmove', { bubbles: true, cancelable: true }));
    vi.advanceTimersByTime(200);
    scrolled(element);
    expect(state(element)).toBeNull();
  });

  it('shows nothing under a finger on a drag handle', () => {
    const { element, inside } = scroller();
    const handle = document.createElement('button');
    handle.style.touchAction = 'none';
    const icon = document.createElement('span');
    handle.append(icon);
    inside.append(handle);
    // The row is dragged by its handle; the list scrolls under it by itself.
    icon.dispatchEvent(new Event('touchmove', { bubbles: true, cancelable: true }));
    scrolled(element);
    expect(state(element)).toBeNull();
  });

  it('follows the keys that scroll, unless something takes them', () => {
    const { element } = scroller();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    scrolled(element);
    expect(state(element)).toBeNull();

    // A row moved with the arrows (the list scrolls along by itself).
    const taken = (event: Event) => event.preventDefault();
    document.body.addEventListener('keydown', taken);
    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }),
    );
    scrolled(element);
    expect(state(element)).toBeNull();
    document.body.removeEventListener('keydown', taken);

    document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'PageDown', bubbles: true }));
    scrolled(element);
    expect(state(element)).toBe('shown');
  });

  it('shows a bar while the mouse is on it or drags it', () => {
    const { element, inside } = scroller();
    // On the content, nothing; on the bar (past the content's 100 px), it shows and stays.
    pointer('pointermove', inside, 50);
    expect(state(element)).toBeNull();
    pointer('pointermove', element, 105);
    expect(state(element)).toBe('shown');
    vi.advanceTimersByTime(SHOWN_MS * 3);
    expect(state(element)).toBe('shown');

    // Dragged: its scrolls show it, and it stays while held, even off the bar.
    pointer('pointerdown', element, 105);
    pointer('pointermove', inside, 50);
    vi.advanceTimersByTime(SHOWN_MS * 2);
    scrolled(element);
    expect(state(element)).toBe('shown');
    pointer('pointerup', inside, 50);
    vi.advanceTimersByTime(SHOWN_MS);
    expect(state(element)).toBe('fading');

    // A finger has no pointer to hover with.
    vi.advanceTimersByTime(FADE_MS);
    pointer('pointermove', element, 105, 'touch');
    expect(state(element)).toBeNull();
  });

  it('lets go of everything when turned off', () => {
    const { element, inside } = scroller();
    inside.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    scrolled(element);
    stop();
    expect(state(element)).toBeNull();
    inside.dispatchEvent(new WheelEvent('wheel', { bubbles: true }));
    scrolled(element);
    expect(state(element)).toBeNull();
    stop = () => undefined;
  });
});
