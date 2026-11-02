import type { VideoChapter } from '@raygun/bindings';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';

type Props = {
  timeMs: number;
  durationMs: number;
  bufferedMs: number;
  chapters: VideoChapter[];
  seekStepMs: number;
  onSeek: (positionMs: number) => void;
};

/**
 * The video's time bar: what's played and buffered, chapter marks, the time under the pointer,
 * and dragging. A range input underneath does the work (and the keys, when it has focus).
 */
export const TimeBar: FC<Props> = ({
  timeMs,
  durationMs,
  bufferedMs,
  chapters,
  seekStepMs,
  onSeek,
}) => {
  const { t } = useTranslation();
  const [dragMs, setDragMs] = useState<number | null>(null);
  const [hover, setHover] = useState<{ x: number; ms: number } | null>(null);
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

  return (
    <div
      className="group relative flex h-5 items-center"
      onMouseMove={(event) => {
        const box = event.currentTarget.getBoundingClientRect();
        if (box.width <= 0 || durationMs <= 0) return;
        const share = Math.min(1, Math.max(0, (event.clientX - box.left) / box.width));
        setHover({ x: event.clientX - box.left, ms: share * durationMs });
      }}
      onMouseLeave={() => setHover(null)}
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
        className="bg-primary pointer-events-none absolute size-3 -translate-x-1/2 rounded-full opacity-0 shadow transition-opacity group-hover:opacity-100"
        style={{ left: percent(shown) }}
      />
      {hover && (
        <span
          aria-hidden
          className="pointer-events-none absolute bottom-6 -translate-x-1/2 rounded-md bg-black/85 px-2 py-1 text-xs whitespace-nowrap tabular-nums"
          style={{ left: hover.x }}
        >
          {hoverChapter?.name ? `${hoverChapter.name} · ` : ''}
          {formatTime(hover.ms)}
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
        className="absolute inset-0 size-full cursor-pointer opacity-0 disabled:cursor-default"
      />
    </div>
  );
};
