import type { FC, KeyboardEvent, PointerEvent as ReactPointerEvent } from 'react';

type ResizeHandleProps = {
  label: string;
  value: number;
  min: number;
  max: number;
  /** Which way a drag to the right moves the size: +1 for a panel on the left, -1 on the right. */
  direction: 1 | -1;
  /** While dragging (live, not saved). */
  onPreview: (value: number) => void;
  /** Once let go (saved). */
  onCommit: (value: number) => void;
  /** Units per screen pixel moved, read when a drag starts (a percentage split's scale). */
  scale?: () => number;
  className?: string;
};

/**
 * A panel edge to drag (M46.8): the rail, the side panel, the full player's split. Arrow keys
 * move it too, Home and End go to its limits; the size is saved when let go.
 */
export const ResizeHandle: FC<ResizeHandleProps> = ({
  label,
  value,
  min,
  max,
  direction,
  onPreview,
  onCommit,
  scale,
  className = '',
}) => {
  const clamp = (next: number) => Math.round(Math.min(max, Math.max(min, next)));
  const start = (event: ReactPointerEvent) => {
    event.preventDefault();
    const from = event.clientX;
    const initial = value;
    const ratio = scale?.() ?? 1;
    let latest = initial;
    const move = (moved: PointerEvent) => {
      latest = clamp(initial + (moved.clientX - from) * direction * ratio);
      onPreview(latest);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      document.body.style.cursor = '';
      onCommit(latest);
    };
    document.body.style.cursor = 'col-resize';
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  const key = (event: KeyboardEvent) => {
    const step = event.shiftKey ? 48 : 16;
    const next =
      event.key === 'ArrowRight'
        ? value + step * direction
        : event.key === 'ArrowLeft'
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
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={start}
      onKeyDown={key}
      className={`group relative z-20 w-2 shrink-0 cursor-col-resize outline-none ${className}`}
    >
      <span className="group-hover:bg-primary/60 group-focus-visible:bg-primary absolute inset-y-0 left-1/2 w-0.5 -translate-x-1/2 rounded-full transition-colors" />
    </div>
  );
};
