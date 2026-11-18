import { Crop, LoaderCircle, Minus, Plus, RotateCw, X } from 'lucide-react';
import { useRef, useState } from 'react';
import type { FC, PointerEvent as ReactPointerEvent, WheelEvent } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';

import { useBackLayer } from '../shell/backLayers';
import { boxBetween, isCrop } from './pictureCrop';
import type { CropBox } from './pictureCrop';

const MIN_ZOOM = 1;
const MAX_ZOOM = 6;
const STEP = 1.5;

const clampZoom = (zoom: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));

/**
 * A picture over the whole window to look at closely: zoomed with two fingers, the wheel, a
 * double tap or the buttons. A screenshot can be cropped there (drag over the part to keep) and
 * read again, `onCrop` getting the part as fractions of the picture. A screenshot not read yet
 * (the import's queue) is cropped or turned before reading: `cropLabel` names what keeping the
 * part does, `onRotate` turns it a quarter clockwise.
 */
export const PictureInspector: FC<{
  src: string;
  title: string;
  onClose: () => void;
  onCrop?: (box: CropBox) => void;
  /** The cropped part is being read (or made). */
  reading?: boolean;
  /** The crop button's words; "Read this part" unless given. */
  cropLabel?: string;
  onRotate?: () => void;
}> = ({ src, title, onClose, onCrop, reading = false, cropLabel, onRotate }) => {
  const { t } = useTranslation();
  useBackLayer(true, onClose);
  const [zoom, setZoom] = useState(1);
  const [cropping, setCropping] = useState(false);
  const [box, setBox] = useState<CropBox | null>(null);
  const picture = useRef<HTMLImageElement>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  // Fingers on the picture, for pinching.
  const fingers = useRef(new Map<number, { x: number; y: number }>());
  const pinch = useRef<{ distance: number; zoom: number } | null>(null);

  const fractionAt = (event: ReactPointerEvent) => {
    const rect = picture.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return { x: 0, y: 0 };
    return {
      x: (event.clientX - rect.left) / rect.width,
      y: (event.clientY - rect.top) / rect.height,
    };
  };
  const spread = () => {
    const [first, second] = [...fingers.current.values()];
    return first && second ? Math.hypot(first.x - second.x, first.y - second.y) : 0;
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    fingers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    if (fingers.current.size === 2) {
      pinch.current = { distance: spread(), zoom };
      start.current = null;
      return;
    }
    if (cropping) {
      event.currentTarget.setPointerCapture?.(event.pointerId);
      start.current = fractionAt(event);
      setBox(null);
    }
  };
  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (fingers.current.has(event.pointerId)) {
      fingers.current.set(event.pointerId, { x: event.clientX, y: event.clientY });
    }
    if (pinch.current && fingers.current.size === 2 && pinch.current.distance > 0) {
      setZoom(clampZoom((pinch.current.zoom * spread()) / pinch.current.distance));
      return;
    }
    if (cropping && start.current) setBox(boxBetween(start.current, fractionAt(event)));
  };
  const onPointerUp = (event: ReactPointerEvent<HTMLDivElement>) => {
    fingers.current.delete(event.pointerId);
    if (fingers.current.size < 2) pinch.current = null;
    start.current = null;
  };
  const onWheel = (event: WheelEvent) => {
    if (!event.ctrlKey) return;
    event.preventDefault();
    setZoom((current) => clampZoom(current * (event.deltaY < 0 ? 1.15 : 1 / 1.15)));
  };

  const button =
    'grid size-10 place-items-center rounded-full bg-black/50 text-white disabled:opacity-40';
  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={title}
      className="motion-fade fixed inset-0 z-[90] flex flex-col bg-black pt-[var(--safe-top)] pr-[var(--safe-right)] pb-[var(--safe-bottom)] pl-[var(--safe-left)]"
    >
      <div className="flex shrink-0 items-center gap-2 p-2 text-white">
        <button type="button" aria-label={t('watch.close')} onClick={onClose} className={button}>
          <X aria-hidden size={20} />
        </button>
        <span className="min-w-0 flex-1 truncate text-sm">{title}</span>
        <button
          type="button"
          aria-label={t('import.zoomOut')}
          disabled={zoom <= MIN_ZOOM}
          onClick={() => setZoom(clampZoom(zoom / STEP))}
          className={button}
        >
          <Minus aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('import.zoomIn')}
          disabled={zoom >= MAX_ZOOM}
          onClick={() => setZoom(clampZoom(zoom * STEP))}
          className={button}
        >
          <Plus aria-hidden size={18} />
        </button>
        {onRotate && (
          <button
            type="button"
            aria-label={t('import.rotate')}
            disabled={reading}
            onClick={onRotate}
            className={button}
          >
            <RotateCw aria-hidden size={18} />
          </button>
        )}
        {onCrop && (
          <button
            type="button"
            aria-pressed={cropping}
            aria-label={t('import.crop')}
            onClick={() => {
              setCropping(!cropping);
              setBox(null);
            }}
            className={`${button} ${cropping ? 'ring-2 ring-white' : ''}`}
          >
            <Crop aria-hidden size={18} />
          </button>
        )}
      </div>
      <div
        data-testid="picture-view"
        className={`min-h-0 flex-1 overflow-auto ${cropping ? 'touch-none' : ''}`}
        onWheel={onWheel}
        onDoubleClick={() => !cropping && setZoom(zoom > 1 ? 1 : 2.5)}
      >
        <div
          className="relative mx-auto"
          style={{ width: `${zoom * 100}%`, cursor: cropping ? 'crosshair' : undefined }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
        >
          <img ref={picture} src={src} alt={title} draggable={false} className="block w-full" />
          {box && (
            <div
              data-testid="crop-box"
              className="pointer-events-none absolute border-2 border-white shadow-[0_0_0_9999px_rgba(0,0,0,0.55)]"
              style={{
                left: `${box.left * 100}%`,
                top: `${box.top * 100}%`,
                width: `${(box.right - box.left) * 100}%`,
                height: `${(box.bottom - box.top) * 100}%`,
              }}
            />
          )}
        </div>
      </div>
      {cropping && onCrop && (
        <div className="flex shrink-0 items-center justify-between gap-2 p-3 text-sm text-white">
          <span className="text-white/70">{t('import.cropHint')}</span>
          <button
            type="button"
            disabled={!box || !isCrop(box) || reading}
            onClick={() => box && onCrop(box)}
            className="bg-primary text-background flex items-center gap-2 rounded-full px-5 py-2 font-semibold disabled:opacity-50"
          >
            {reading && <LoaderCircle aria-hidden size={16} className="animate-spin" />}
            {reading ? t('import.reading') : (cropLabel ?? t('import.readCrop'))}
          </button>
        </div>
      )}
    </div>,
    document.body,
  );
};
