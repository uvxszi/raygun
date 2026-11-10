import { useRef } from 'react';
import type { FC, PointerEvent as ReactPointerEvent } from 'react';

import { dragIntent, shareAlong, valueAlong } from './sliderDrag';
import type { SliderAxis } from './sliderDrag';

type DragSliderProps = {
  axis: SliderAxis;
  /** Off, it takes no presses and the page scrolls over it. */
  enabled: boolean;
  label: string;
  valueText: string;
  min: number;
  max: number;
  step: number;
  value: number;
  onValue: (value: number) => void;
  /** A double click or tap puts it back (a band to 0 dB). */
  onReset?: () => void;
  /** The box around the input, which takes its presses. */
  boxClassName: string;
  /** The input's own look. */
  className: string;
};

/** A press on a slider's box: where it started, and whether it slides the slider yet. */
type Press = { id: number; x: number; y: number; sliding: boolean };

/**
 * A range input whose box takes the presses (the input itself takes none), so a finger scrolling
 * the page over it never moves it: a mouse sets it at once and drags it, a finger or pen only once
 * it clearly moves along the slider (then the box captures it), and one that moves across first is
 * left to the page. `touch-action` keeps the page's scroll across the slider while it's on, and all
 * of it while it's off; nothing is ever prevented. Keys and screen readers use the input as usual.
 */
export const DragSlider: FC<DragSliderProps> = ({
  axis,
  enabled,
  label,
  valueText,
  min,
  max,
  step,
  value,
  onValue,
  onReset,
  boxClassName,
  className,
}) => {
  const input = useRef<HTMLInputElement>(null);
  const press = useRef<Press | null>(null);

  const follow = (event: ReactPointerEvent<HTMLElement>) => {
    const element = input.current;
    if (!element) return;
    const rtl = axis === 'x' && getComputedStyle(element).direction === 'rtl';
    const share = shareAlong(
      { x: event.clientX, y: event.clientY },
      element.getBoundingClientRect(),
      axis,
      rtl,
    );
    const next = valueAlong(share, min, max, step);
    if (next !== value) onValue(next);
  };
  const slide = (event: ReactPointerEvent<HTMLElement>, current: Press) => {
    current.sliding = true;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    follow(event);
  };
  const letGo = (event: ReactPointerEvent<HTMLElement>) => {
    if (press.current?.id === event.pointerId) press.current = null;
  };

  return (
    <div
      data-slider-box=""
      style={{ touchAction: enabled ? (axis === 'y' ? 'pan-x' : 'pan-y') : 'auto' }}
      onPointerDown={(event) => {
        if (!enabled || (event.pointerType === 'mouse' && event.button !== 0)) return;
        const current = { id: event.pointerId, x: event.clientX, y: event.clientY, sliding: false };
        press.current = current;
        // A mouse means it: the slider jumps there and follows, as a native one does.
        if (event.pointerType === 'mouse') {
          input.current?.focus({ preventScroll: true });
          slide(event, current);
        }
      }}
      onPointerMove={(event) => {
        const current = press.current;
        if (!enabled || !current || current.id !== event.pointerId) return;
        if (current.sliding) {
          follow(event);
          return;
        }
        const intent = dragIntent(event.clientX - current.x, event.clientY - current.y, axis);
        if (intent === 'pass') press.current = null;
        else if (intent === 'slide') slide(event, current);
      }}
      onPointerUp={letGo}
      onPointerCancel={letGo}
      onLostPointerCapture={letGo}
      onDoubleClick={enabled ? onReset : undefined}
      className={`${boxClassName} ${enabled ? 'cursor-pointer' : ''}`}
    >
      <input
        ref={input}
        type="range"
        aria-label={label}
        aria-valuetext={valueText}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={!enabled}
        onChange={(event) => onValue(Number(event.target.value))}
        className={`pointer-events-none ${className}`}
      />
    </div>
  );
};
