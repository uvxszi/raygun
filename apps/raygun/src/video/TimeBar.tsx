import type { VideoChapter } from '@raygun/bindings';
import { useRef, useState } from 'react';
import type { FC, PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';
import { DRAG_SLOP_PX, dragTarget } from './seekDrag';

type Props = {
  timeMs: number;
  durationMs: number;
  bufferedMs: number;
  chapters: VideoChapter[];
  seekStepMs: number;
  onSeek: (positionMs: number) => void;
  /**
   * Seek by dragging (Settings → Player): a click doesn't jump; holding and dragging moves from
   * where the video is, telling `onDrag` where it has got to (null once it ends).
   */
  dragOnly?: boolean;
  onDrag?: (positionMs: number | null) => void;
};

/**
 * The video's time bar: what's played and buffered, chapter marks, the time under the pointer,
 * and dragging. A range input underneath does the work (and the keys, when it has focus); while
 * seeking by dragging, the pointer is the bar's own and the input keeps only the keys.
 */
export const TimeBar: FC<Props> = ({
  timeMs,
  durationMs,
  bufferedMs,
  chapters,
  seekStepMs,
  onSeek,
  dragOnly = false,
  onDrag,
}) => {
  const { t } = useTranslation();
  const [dragMs, setDragMs] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; ms: number } | null>(null);
  /** A press while seeking by dragging: where it began, and whether it has moved enough. */
  const press = useRef<{ x: number; fromMs: number; moving: boolean } | null>(null);
  const length = Math.max(durationMs, 1);
  const shown = dragMs ?? timeMs;
  const percent = (ms: number) => `${Math.min(100, Math.max(0, (ms / length) * 100))}%`;
  const hoverChapter = hover
    ? [...chapters].reverse().find((chapter) => chapter.startMs <= hover.ms)
    : undefined;

  const commit = () => {
    if (dragMs !== null) {
      onSeek(dragMs);
      setDragMs(null);
    }
  };

  const pressDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!dragOnly || durationMs <= 0 || event.button !== 0) return;
    event.currentTarget.setPointerCapture?.(event.pointerId);
    press.current = { x: event.clientX, fromMs: timeMs, moving: false };
  };
  const pressMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const pressed = press.current;
    if (!pressed) return;
    const moved = event.clientX - pressed.x;
    if (!pressed.moving) {
      if (Math.abs(moved) <= DRAG_SLOP_PX) return;
      // From here on the travel counts from where the drag began.
      press.current = { x: event.clientX, fromMs: pressed.fromMs, moving: true };
      setDragMs(pressed.fromMs);
      onDrag?.(pressed.fromMs);
      return;
    }
    const width = event.currentTarget.getBoundingClientRect().width;
    const target = dragTarget(pressed.fromMs, moved, width, durationMs);
    setDragMs(target);
    onDrag?.(target);
  };
  const pressUp = (cancelled: boolean) => {
    const pressed = press.current;
    press.current = null;
    if (!pressed?.moving) return;
    const target = dragMs ?? pressed.fromMs;
    setDragMs(null);
    onDrag?.(null);
    onSeek(cancelled ? pressed.fromMs : target);
  };
  const dragging = dragOnly && dragMs !== null;

  return (
    <div
      data-testid="time-bar"
      className={`group relative flex h-5 items-center ${dragOnly ? 'cursor-grab touch-none active:cursor-grabbing' : ''}`}
      onMouseMove={(event) => {
        const box = event.currentTarget.getBoundingClientRect();
        if (box.width <= 0 || durationMs <= 0) return;
        const share = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
        setHover({ x: event.clientX - box.left, ms: share * durationMs });
      }}
      onMouseLeave={() => setHover(null)}
      onPointerDown={pressDown}
      onPointerMove={pressMove}
      onPointerUp={() => pressUp(false)}
      onPointerCancel={() => pressUp(true)}
    >
      <div className="pointer-events-none relative h-1 w-full overflow-hidden rounded-full bg-white/25 transition-[height] group-hover:h-1.5">
        <div
          className="absolute inset-y-0 left-0 bg-white/40"
          style={{ width: percent(bufferedMs) }}
        />
        <div className="bg-primary absolute inset-y-0 left-0" style={{ width: percent(shown) }} />
        {chapters
          .filter((chapter) => chapter.startMs > 0)
          .map((chapter) => (
            <span
              key={chapter.startMs}
              aria-hidden
              className="absolute inset-y-0 w-0.5 bg-black/70"
              style={{ left: percent(chapter.startMs) }}
            />
          ))}
      </div>
      <span
        aria-hidden
        className={`bg-primary pointer-events-none absolute size-3 -translate-x-1/2 rounded-full shadow transition-opacity group-hover:opacity-100 ${dragging ? 'opacity-100' : 'opacity-0'}`}
        style={{ left: percent(shown) }}
      />
      {/* While dragging, the time under the thumb; else the time under the pointer. */}
      {(dragging || hover) && (
        <span
          aria-hidden
          data-testid="time-bar-tip"
          className="pointer-events-none absolute bottom-6 -translate-x-1/2 rounded-md bg-black/85 px-2 py-1 text-xs whitespace-nowrap tabular-nums"
          style={{ left: dragging ? percent(shown) : hover?.x }}
        >
          {!dragging && hoverChapter?.name ? `${hoverChapter.name} · ` : ''}
          {formatTime(dragging ? shown : (hover?.ms ?? 0))}
        </span>
      )}
      <input
        type="range"
        aria-label={t('player.seek')}
        aria-valuetext={`${formatTime(shown)} / ${formatTime(durationMs)}`}
        min={0}
        max={length}
        step={seekStepMs}
        value={Math.min(shown, length)}
        disabled={durationMs <= 0}
        onChange={(event) => setDragMs(Number(event.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
        className={`absolute inset-0 size-full opacity-0 disabled:cursor-default ${
          dragOnly ? 'pointer-events-none' : 'cursor-pointer'
        }`}
      />
    </div>
  );
};
