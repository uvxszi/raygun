import { useEffect, useRef } from 'react';
import type { FC } from 'react';

import { picturesFade } from '../shell/imageMemory';

/** The canvas's side, px: a cover this small and blurred is all a full-screen wash needs. */
const SIDE = 48;
/** The blur in the canvas's px: about what a 64 px CSS blur gives the full-size cover. */
const BLUR_PX = 4;
/** Drawn this far past each edge (px), so the blur doesn't fade the edges out. */
const BLEED_PX = 8;
/** How long a new cover's wash takes to fade in over the last one (ms), by motion level. */
const FADE_MS = { lively: 450, calm: 300 } as const;

/**
 * The playing cover as a soft wash behind the full player: drawn once per cover, small and
 * blurred, into a canvas the screen stretches. Nothing is left to blur afterwards, so the sheet
 * moving under a finger and the seek bar changing over it cost no filter work (a CSS blur over the
 * whole screen was worked out again for every frame). The cover is cut square from its middle, as
 * `object-cover` would; one that fails to load leaves the wash empty. A new cover's wash fades in
 * over the last one's (kept in a second canvas under it) where things may move. Without a 2D
 * canvas it draws nothing.
 */
export const CoverWash: FC<{ url: string; className?: string }> = ({ url, className = '' }) => {
  const backRef = useRef<HTMLCanvasElement>(null);
  const frontRef = useRef<HTMLCanvasElement>(null);
  /** Whether the front canvas holds a cover (one to keep under the next while it fades in). */
  const drawn = useRef(false);
  useEffect(() => {
    const front = frontRef.current;
    const context = front?.getContext('2d');
    if (!front || !context) return;
    const back = backRef.current?.getContext('2d') ?? null;
    const image = new Image();
    image.decoding = 'async';
    let current = true;
    image.onload = () => {
      if (!current) return;
      const side = Math.min(image.naturalWidth, image.naturalHeight);
      const level = document.documentElement.dataset.motion;
      const fade =
        picturesFade() && typeof front.animate === 'function'
          ? FADE_MS[level === 'calm' ? 'calm' : 'lively']
          : 0;
      if (back) {
        back.clearRect(0, 0, SIDE, SIDE);
        // What's on screen stays under the new wash while it fades in.
        if (fade > 0 && drawn.current) back.drawImage(front, 0, 0);
      }
      context.clearRect(0, 0, SIDE, SIDE);
      drawn.current = side > 0;
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
      if (fade > 0) {
        front.animate([{ opacity: 0 }, { opacity: 1 }], { duration: fade, easing: 'ease-out' });
      }
    };
    image.onerror = () => {
      if (!current) return;
      context.clearRect(0, 0, SIDE, SIDE);
      back?.clearRect(0, 0, SIDE, SIDE);
      drawn.current = false;
    };
    image.src = url;
    return () => {
      current = false;
      image.onload = null;
      image.onerror = null;
    };
  }, [url]);
  return (
    <div aria-hidden data-testid="cover-wash" className={`relative ${className}`}>
      <canvas
        ref={backRef}
        width={SIDE}
        height={SIDE}
        className="absolute inset-0 size-full object-cover"
      />
      <canvas
        ref={frontRef}
        width={SIDE}
        height={SIDE}
        className="absolute inset-0 size-full object-cover"
      />
    </div>
  );
};
