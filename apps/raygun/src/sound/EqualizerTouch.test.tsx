import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { EqualizerPanel } from './EqualizerPanel';
import { dragIntent, shareAlong, valueAlong } from './sliderDrag';
import { SOUND_DEFAULTS } from './soundPrefs';

/** The box that takes a slider's presses. */
const boxOf = (slider: HTMLElement) => slider.closest<HTMLElement>('[data-slider-box]')!;

/** A band's track as a phone lays it out: 24 × 160 px from (0, 100). */
const layOut = (slider: HTMLElement) => {
  slider.getBoundingClientRect = () =>
    ({
      x: 0,
      y: 100,
      left: 0,
      top: 100,
      right: 24,
      bottom: 260,
      width: 24,
      height: 160,
    }) as DOMRect;
};

describe('Equalizer sliders under a finger', () => {
  const capture = vi.fn();
  beforeEach(() => {
    capture.mockReset();
    // jsdom has no pointer capture.
    HTMLElement.prototype.setPointerCapture = capture;
  });
  afterEach(() => {
    delete (HTMLElement.prototype as { setPointerCapture?: unknown }).setPointerCapture;
  });

  it('lets a finger scroll the page over the bands and the preamp while it is off', async () => {
    const user = userEvent.setup();
    const change = vi.fn();
    render(<EqualizerPanel sound={SOUND_DEFAULTS} change={change} />);
    const band = screen.getByRole('slider', { name: '1kHz band' });
    const preamp = screen.getByRole('slider', { name: 'Preamp' });
    layOut(band);
    // Every direction is the page's.
    expect(boxOf(band).style.touchAction).toBe('auto');
    expect(boxOf(preamp).style.touchAction).toBe('auto');

    // Up along the band, as a scroll would go: nothing taken, nothing prevented.
    await user.pointer([
      { keys: '[TouchA>]', target: boxOf(band), coords: { clientX: 12, clientY: 220 } },
      { pointerName: 'TouchA', target: boxOf(band), coords: { clientX: 12, clientY: 160 } },
      { pointerName: 'TouchA', target: boxOf(band), coords: { clientX: 12, clientY: 110 } },
      { keys: '[/TouchA]', target: boxOf(band) },
    ]);
    expect(fireEvent.touchMove(boxOf(band))).toBe(true);
    expect(fireEvent.pointerMove(boxOf(band), { clientX: 12, clientY: 90 })).toBe(true);
    expect(capture).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
  });

  it('takes a finger on a band only once it clearly moves along it', async () => {
    const user = userEvent.setup();
    const change = vi.fn();
    render(<EqualizerPanel sound={{ ...SOUND_DEFAULTS, eqEnabled: true }} change={change} />);
    const band = screen.getByRole('slider', { name: '1kHz band' });
    const box = boxOf(band);
    layOut(band);
    // Across the slider stays the page's: sideways over the bands, up and down over the preamp.
    expect(box.style.touchAction).toBe('pan-x');
    expect(boxOf(screen.getByRole('slider', { name: 'Preamp' })).style.touchAction).toBe('pan-y');

    // Moving across first leaves it alone, even when the finger turns along it later.
    await user.pointer([
      { keys: '[TouchA>]', target: box, coords: { clientX: 12, clientY: 180 } },
      { pointerName: 'TouchA', target: box, coords: { clientX: 40, clientY: 183 } },
      { pointerName: 'TouchA', target: box, coords: { clientX: 40, clientY: 120 } },
      { keys: '[/TouchA]', target: box },
    ]);
    expect(capture).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();

    // A press and a wobble say nothing yet; up along the band takes it, and it follows.
    await user.pointer([
      { keys: '[TouchA>]', target: box, coords: { clientX: 12, clientY: 180 } },
      { pointerName: 'TouchA', target: box, coords: { clientX: 14, clientY: 176 } },
    ]);
    expect(capture).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
    await user.pointer({
      pointerName: 'TouchA',
      target: box,
      coords: { clientX: 14, clientY: 120 },
    });
    expect(capture).toHaveBeenCalledTimes(1);
    expect(change).toHaveBeenLastCalledWith({
      eqBands: [0, 0, 0, 0, 0, 100, 0, 0, 0, 0],
      eqPreset: 'custom',
    });
    await user.pointer({ keys: '[/TouchA]', target: box });
  });

  it('reads a press as along or across the slider, and its place on the track', () => {
    expect(dragIntent(2, -5, 'y')).toBeNull();
    expect(dragIntent(2, -12, 'y')).toBe('slide');
    expect(dragIntent(14, 9, 'y')).toBe('pass');
    expect(dragIntent(14, 9, 'x')).toBe('slide');
    const track = { left: 0, top: 100, width: 24, height: 160 };
    expect(shareAlong({ x: 12, y: 252 }, track, 'y')).toBe(0);
    expect(shareAlong({ x: 12, y: 108 }, track, 'y')).toBe(1);
    expect(shareAlong({ x: 12, y: 40 }, track, 'y')).toBe(1);
    const wide = { left: 0, top: 0, width: 216, height: 20 };
    expect(shareAlong({ x: 8, y: 10 }, wide, 'x')).toBe(0);
    expect(shareAlong({ x: 8, y: 10 }, wide, 'x', true)).toBe(1);
    expect(valueAlong(0.5, -120, 120, 5)).toBe(0);
    expect(valueAlong(0.91, -120, 120, 5)).toBe(100);
    expect(valueAlong(1.2, -120, 120, 5)).toBe(120);
  });
});
