import type { PictureBox } from '../pictureBox';
import { WATERMARK_SCALE } from './watermarkMath';
import type { Watermark } from './watermarkMath';

export type Point = { x: number; y: number };
export type Box = Pick<Watermark, 'x' | 'y' | 'width' | 'height'>;

/** A box narrower or lower than this share of the picture is a stray click. */
const SMALLEST = 0.01;

/**
 * The box a drag from `start` to `end` (the player's pixels) draws over the picture, in
 * ten-thousandths of it from its top left, kept inside it; null for a click.
 */
export const boxFromDrag = (start: Point, end: Point, picture: PictureBox): Box | null => {
  if (picture.width <= 0 || picture.height <= 0) return null;
  const share = (point: Point) => ({
    x: Math.min(1, Math.max(0, (point.x - picture.left) / picture.width)),
    y: Math.min(1, Math.max(0, (point.y - picture.top) / picture.height)),
  });
  const from = share(start);
  const to = share(end);
  const left = Math.min(from.x, to.x);
  const top = Math.min(from.y, to.y);
  const width = Math.abs(to.x - from.x);
  const height = Math.abs(to.y - from.y);
  if (width < SMALLEST || height < SMALLEST) return null;
  const x = Math.round(left * WATERMARK_SCALE);
  const y = Math.round(top * WATERMARK_SCALE);
  return {
    x,
    y,
    width: Math.min(WATERMARK_SCALE - x, Math.round(width * WATERMARK_SCALE)),
    height: Math.min(WATERMARK_SCALE - y, Math.round(height * WATERMARK_SCALE)),
  };
};

/** A box in the player's pixels, for drawing it. */
export const shownBox = (box: Box, picture: PictureBox) => ({
  left: picture.left + (box.x / WATERMARK_SCALE) * picture.width,
  top: picture.top + (box.y / WATERMARK_SCALE) * picture.height,
  width: (box.width / WATERMARK_SCALE) * picture.width,
  height: (box.height / WATERMARK_SCALE) * picture.height,
});
