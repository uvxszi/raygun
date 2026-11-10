import { useEffect, useRef } from 'react';
import type { FC } from 'react';

/** The canvas's side, px: a cover this small and blurred is all a full-screen wash needs. */
const SIDE = 48;
/** The blur in the canvas's px: about what a 64 px CSS blur gives the full-size cover. */
const BLUR_PX = 4;
/** Drawn this far past each edge (px), so the blur doesn't fade the edges out. */
const BLEED_PX = 8;

/**
 * The playing cover as a soft wash behind the full player: drawn once per cover, small and
 * blurred, into a canvas the screen stretches. Nothing is left to blur afterwards, so the sheet
 * moving under a finger and the seek bar changing over it cost no filter work (a CSS blur over the
 * whole screen was worked out again for every frame). The cover is cut square from its middle, as
 * `object-cover` would; one that fails to load leaves the wash empty. Without a 2D canvas it
 * draws nothing.
 */
export const CoverWash: FC<{ url: string; className?: string }> = ({ url, className = '' }) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const context = canvasRef.current?.getContext('2d');
    if (!context) return;
    const image = new Image();
    image.decoding = 'async';
    let current = true;
    image.onload = () => {
      if (!current) return;
      const side = Math.min(image.naturalWidth, image.naturalHeight);
      context.clearRect(0, 0, SIDE, SIDE);
      if (side <= 0) return;
      context.filter = `blur(${BLUR_PX}px)`;
      context.drawImage(
        image,
        (image.naturalWidth - side) / 2,
        (image.naturalHeight - side) / 2,
        side,
        side,
        -BLEED_PX,
        -BLEED_PX,
        SIDE + BLEED_PX * 2,
        SIDE + BLEED_PX * 2,
      );
      context.filter = 'none';
    };
    image.onerror = () => {
      if (current) context.clearRect(0, 0, SIDE, SIDE);
    };
    image.src = url;
    return () => {
      current = false;
      image.onload = null;
      image.onerror = null;
    };
  }, [url]);
  return (
    <canvas
      ref={canvasRef}
      width={SIDE}
      height={SIDE}
      aria-hidden
      data-testid="cover-wash"
      className={className}
    />
  );
};
