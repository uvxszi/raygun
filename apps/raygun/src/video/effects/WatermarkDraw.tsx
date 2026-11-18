import { useEffect, useRef, useState } from 'react';
import type { FC, PointerEvent as ReactPointerEvent } from 'react';
import { useTranslation } from 'react-i18next';

import type { PictureBox } from '../pictureBox';
import { boxFromDrag, shownBox } from './watermarkBox';
import type { Box, Point } from './watermarkBox';

type Props = {
  picture: PictureBox | null;
  /** The box the title has now, shown until a new one is drawn. */
  current: Box | null;
  onDone: (box: Box) => void;
  onCancel: () => void;
};

/**
 * Drawing the watermark remover's box over the desktop player's picture, the player's controls
 * hidden meanwhile: a drag draws it (another drag draws it again), Done keeps it, Cancel or Escape
 * leaves the box as it was.
 */
export const WatermarkDraw: FC<Props> = ({ picture, current, onDone, onCancel }) => {
  const { t } = useTranslation();
  const layer = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ start: Point; end: Point } | null>(null);
  /** The box drawn so far, kept until Done. */
  const [drawnBox, setDrawnBox] = useState<Box | null>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  }, [onCancel]);

  const at = (event: ReactPointerEvent): Point => {
    const rect = layer.current?.getBoundingClientRect();
    return { x: event.clientX - (rect?.left ?? 0), y: event.clientY - (rect?.top ?? 0) };
  };
  const dragging = picture && drag ? boxFromDrag(drag.start, drag.end, picture) : null;
  const outline = picture ? (dragging ?? drawnBox ?? current) : null;
  const rect = outline && picture ? shownBox(outline, picture) : null;

  return (
    <div
      ref={layer}
      data-testid="watermark-draw"
      role="application"
      aria-label={t('videoPlayer.watermarkDrawHint')}
      className="absolute inset-0 z-30 cursor-crosshair touch-none bg-black/30"
      onPointerDown={(event) => {
        event.stopPropagation();
        layer.current?.setPointerCapture?.(event.pointerId);
        const point = at(event);
        setDrag({ start: point, end: point });
      }}
      onPointerMove={(event) => {
        if (!drag) return;
        const point = at(event);
        setDrag({ ...drag, end: point });
      }}
      onPointerUp={(event) => {
        event.stopPropagation();
        if (!drag || !picture) return;
        const box = boxFromDrag(drag.start, at(event), picture);
        setDrag(null);
        if (box) setDrawnBox(box);
      }}
      onClick={(event) => event.stopPropagation()}
    >
      <p className="pointer-events-none absolute inset-x-0 top-6 text-center text-sm font-medium text-white drop-shadow">
        {t('videoPlayer.watermarkDrawHint')}
      </p>
      {rect && (
        <div
          data-testid="watermark-box"
          className="pointer-events-none absolute rounded-sm border-2 border-white shadow-[0_0_0_1px_rgba(0,0,0,0.6)]"
          style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }}
        />
      )}
      <div
        className="absolute inset-x-0 bottom-8 flex justify-center gap-3"
        onPointerDown={(event) => event.stopPropagation()}
        onPointerUp={(event) => event.stopPropagation()}
      >
        <button
          type="button"
          onClick={onCancel}
          className="rounded-full bg-black/60 px-4 py-2 text-sm font-medium text-white hover:bg-black/80"
        >
          {t('videoPlayer.cancel')}
        </button>
        <button
          type="button"
          disabled={!drawnBox}
          onClick={() => drawnBox && onDone(drawnBox)}
          className="rounded-full bg-white px-4 py-2 text-sm font-semibold text-black disabled:opacity-40"
        >
          {t('videoPlayer.done')}
        </button>
      </div>
    </div>
  );
};
