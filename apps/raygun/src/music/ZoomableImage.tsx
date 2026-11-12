import { Maximize, ZoomIn, ZoomOut } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { fadingImage } from '../shell/imageMemory';

const MIN_SCALE = 1;
const MAX_SCALE = 8;
const STEP = 1.5;

type View = { scale: number; x: number; y: number };

const FITTED: View = { scale: 1, x: 0, y: 0 };

/** Keeps the picture inside the frame: no panning past its edges, none at all when fitted. */
const clamp = (view: View, frame: HTMLElement | null): View => {
  const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, view.scale));
  if (scale === 1 || !frame) return { scale, x: 0, y: 0 };
  const limitX = ((scale - 1) * frame.clientWidth) / 2;
  const limitY = ((scale - 1) * frame.clientHeight) / 2;
  return {
    scale,
    x: Math.min(limitX, Math.max(-limitX, view.x)),
    y: Math.min(limitY, Math.max(-limitY, view.y)),
  };
};

/**
 * A picture to look at closely: the wheel or a pinch zooms (toward the pointer), dragging pans, a
 * double click or tap zooms in or back out. Only `transform` changes, so zooming stays smooth.
 */
export const ZoomableImage: FC<{
  src: string;
  alt: string;
  onLoad?: (size: { width: number; height: number }) => void;
  onError?: () => void;
}> = ({ src, alt, onLoad, onError }) => {
  const { t } = useTranslation();
  const frame = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<View>(FITTED);
  const pointers = useRef(new Map<number, { x: number; y: number }>());
  const pinch = useRef<{ distance: number; view: View } | null>(null);

  /** Zooms by `factor` around a point given from the frame's centre. */
  const zoomAt = (current: View, factor: number, offsetX = 0, offsetY = 0): View => {
    const scale = Math.min(MAX_SCALE, Math.max(MIN_SCALE, current.scale * factor));
    const ratio = scale / current.scale;
    return clamp(
      {
        scale,
        x: offsetX - (offsetX - current.x) * ratio,
        y: offsetY - (offsetY - current.y) * ratio,
      },
      frame.current,
    );
  };

  const fromCentre = (clientX: number, clientY: number) => {
    const box = frame.current?.getBoundingClientRect();
    if (!box) return { x: 0, y: 0 };
    return { x: clientX - box.left - box.width / 2, y: clientY - box.top - box.height / 2 };
  };

  // React's wheel listener is passive; this one keeps the sheet from scrolling while zooming.
  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const point = fromCentre(event.clientX, event.clientY);
      setView((current) => zoomAt(current, Math.exp(-event.deltaY * 0.0015), point.x, point.y));
    };
    element.addEventListener('wheel', onWheel, { passive: false });
    return () => element.removeEventListener('wheel', onWheel);
  }, []);

  /** The two fingers of a pinch: how far apart they are and the point between them. */
  const fingers = () => {
    const [first, second] = [...pointers.current.values()];
    if (!first || !second) return null;
    return {
      distance: Math.hypot(first.x - second.x, first.y - second.y),
      middle: fromCentre((first.x + second.x) / 2, (first.y + second.y) / 2),
    };
  };

  const onPointerDown = (event: ReactPointerEvent) => {
    pointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    event.currentTarget.setPointerCapture?.(event.pointerId);
    const pair = pointers.current.size === 2 ? fingers() : null;
    if (pair) pinch.current = { distance: pair.distance, view };
  };

  const onPointerMove = (event: ReactPointerEvent) => {
    const last = pointers.current.get(event.pointerId);
    if (!last) return;
    pointers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    const pair = pointers.current.size === 2 && pinch.current ? fingers() : null;
    if (pair && pinch.current) {
      const start = pinch.current;
      const factor = pair.distance / Math.max(1, start.distance);
      setView(zoomAt(start.view, factor, pair.middle.x, pair.middle.y));
      return;
    }
    const dx = event.clientX - last.x;
    const dy = event.clientY - last.y;
    setView((current) =>
      current.scale === 1
        ? current
        : clamp({ ...current, x: current.x + dx, y: current.y + dy }, frame.current),
    );
  };

  const onPointerUp = (event: ReactPointerEvent) => {
    pointers.current.delete(event.pointerId);
    if (pointers.current.size < 2) pinch.current = null;
  };

  const button =
    'bg-surface/80 hover:bg-surface grid size-9 place-items-center rounded-full backdrop-blur disabled:opacity-40';
  const fading = fadingImage(src, true);

  return (
    <div className="relative">
      <div
        ref={frame}
        data-testid="zoomable-image"
        data-scale={view.scale.toFixed(2)}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
        onDoubleClick={(event) => {
          const point = fromCentre(event.clientX, event.clientY);
          setView((current) =>
            current.scale > 1 ? FITTED : zoomAt(current, 2.5, point.x, point.y),
          );
        }}
        className={`bg-background flex h-[min(60vh,40rem)] touch-none items-center justify-center overflow-hidden rounded-xl select-none ${
          view.scale > 1 ? 'cursor-grab active:cursor-grabbing' : 'cursor-zoom-in'
        }`}
      >
        <img
          src={src}
          alt={alt}
          draggable={false}
          // Fades in once loaded and decoded, unless it has shown before.
          {...fading}
          onLoad={(event) => {
            fading.onLoad(event);
            onLoad?.({
              width: event.currentTarget.naturalWidth,
              height: event.currentTarget.naturalHeight,
            });
          }}
          onError={onError}
          style={{ transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})` }}
          className="max-h-full max-w-full object-contain will-change-transform"
        />
      </div>
      <div className="absolute right-2 bottom-2 flex gap-1.5">
        <button
          type="button"
          aria-label={t('cover.zoomOut')}
          disabled={view.scale <= MIN_SCALE}
          onClick={() => setView((current) => zoomAt(current, 1 / STEP))}
          className={button}
        >
          <ZoomOut aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('cover.zoomIn')}
          disabled={view.scale >= MAX_SCALE}
          onClick={() => setView((current) => zoomAt(current, STEP))}
          className={button}
        >
          <ZoomIn aria-hidden size={18} />
        </button>
        {view.scale > 1 && (
          <button
            type="button"
            aria-label={t('cover.fit')}
            onClick={() => setView(FITTED)}
            className={button}
          >
            <Maximize aria-hidden size={18} />
          </button>
        )}
      </div>
    </div>
  );
};
