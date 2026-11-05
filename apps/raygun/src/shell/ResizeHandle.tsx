import type { FC, KeyboardEvent, PointerEvent as ReactPointerEvent } from 'react';

type ResizeHandleProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  /**
   * Which way a drag to the right (or down, `across`) moves the size: +1 for a panel on the left
   * (or above), -1 on the right (or below).
   */
  direction: 1 | -1;
  /** While dragging (live, not saved). */
  onPreview: (value: number) => void;
  /** Once let go (saved). */
  onCommit: (value: number) => void;
  /** A double click: back to the panel's own size. */
  onReset?: () => void;
  /** Units per screen pixel moved, read when a drag starts (a percentage split's scale). */
  scale?: () => number;
  /** Between panels one above the other: dragged up and down, a line across. */
  across?: boolean;
  className?: string;
};

/**
 * A panel edge to drag (M46.8): the rail, the side panel, the full player's split, the rail's
 * tabs and quick access. Arrow keys move it too, Home and End go to its limits; the size is saved
 * when let go.
 */
export const ResizeHandle: FC<ResizeHandleProps> = ({
  label,
  value,
  min,
  max,
  direction,
  onPreview,
  onCommit,
  onReset,
  scale,
  across = false,
  className = '',
}) => {
  const clamp = (next: number) => Math.round(Math.min(max, Math.max(min, next)));
  const at = (event: { clientX: number; clientY: number }) =>
    across ? event.clientY : event.clientX;
  const start = (event: ReactPointerEvent) => {
    event.preventDefault();
    const from = at(event);
    const initial = value;
    const ratio = scale?.() ?? 1;
    let latest = initial;
    // A press that didn't move (a click, half a double click) saves nothing.
    let dragged = false;
    const move = (moved: PointerEvent) => {
      dragged = true;
      latest = clamp(initial + (at(moved) - from) * direction * ratio);
      onPreview(latest);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      document.body.style.cursor = '';
      if (dragged) onCommit(latest);
    };
    document.body.style.cursor = across ? 'row-resize' : 'col-resize';
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  const [grow, shrink] = across ? ['ArrowDown', 'ArrowUp'] : ['ArrowRight', 'ArrowLeft'];
  const key = (event: KeyboardEvent) => {
    const step = event.shiftKey ? 48 : 16;
    const next =
      event.key === grow
        ? value + step * direction
        : event.key === shrink
          ? value - step * direction
          : event.key === 'Home'
            ? min
            : event.key === 'End'
              ? max
              : null;
    if (next === null) return;
    event.preventDefault();
    onCommit(clamp(next));
  };
  return (
    <div
      role="separator"
      aria-orientation={across ? 'horizontal' : 'vertical'}
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={start}
      onKeyDown={key}
      onDoubleClick={onReset}
      className={`group relative z-20 shrink-0 outline-none ${
        across ? 'h-2 cursor-row-resize' : 'w-2 cursor-col-resize'
      } ${className}`}
    >
      <span
        className={`group-hover:bg-primary/60 group-focus-visible:bg-primary absolute rounded-full transition-colors ${
          across
            ? 'inset-x-0 top-1/2 h-0.5 -translate-y-1/2'
            : 'inset-y-0 left-1/2 w-0.5 -translate-x-1/2'
        }`}
      />
    </div>
  );
};
