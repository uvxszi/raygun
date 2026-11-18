/** A part of a picture, in fractions of its width and height (0 to 1). */
export type CropBox = { left: number; top: number; right: number; bottom: number };

const clamp = (value: number) => Math.min(1, Math.max(0, value));

/** The box between two points (fractions, in any order), kept within the picture. */
export const boxBetween = (
  from: { x: number; y: number },
  to: { x: number; y: number },
): CropBox => ({
  left: clamp(Math.min(from.x, to.x)),
  top: clamp(Math.min(from.y, to.y)),
  right: clamp(Math.max(from.x, to.x)),
  bottom: clamp(Math.max(from.y, to.y)),
});

/** Boxes smaller than this share of a side are taps, not crops. */
const MIN_SIDE = 0.03;

/** Whether a box is big enough to read on its own. */
export const isCrop = (box: CropBox) =>
  box.right - box.left >= MIN_SIDE && box.bottom - box.top >= MIN_SIDE;

/** The box in a picture's pixels: left, top, width, height (at least 1 each). */
export const pixelsOf = (box: CropBox, width: number, height: number) => {
  const left = Math.floor(box.left * width);
  const top = Math.floor(box.top * height);
  return {
    left,
    top,
    width: Math.max(1, Math.round((box.right - box.left) * width)),
    height: Math.max(1, Math.round((box.bottom - box.top) * height)),
  };
};

/** A new PNG of the part of `file` inside `box`, named after it. */
export const cropPicture = async (file: Blob, box: CropBox, name: string): Promise<File> => {
  const bitmap = await createImageBitmap(file);
  const part = pixelsOf(box, bitmap.width, bitmap.height);
  const canvas = document.createElement('canvas');
  canvas.width = part.width;
  canvas.height = part.height;
  canvas
    .getContext('2d')
    ?.drawImage(
      bitmap,
      part.left,
      part.top,
      part.width,
      part.height,
      0,
      0,
      part.width,
      part.height,
    );
  bitmap.close();
  const blob = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob(
      (made) => (made ? resolve(made) : reject(new Error('The picture could not be cropped'))),
      'image/png',
    ),
  );
  return new File([blob], name.replace(/(?:-cropped)?\.[^.]+$/, '') + '-cropped.png', {
    type: 'image/png',
    lastModified: Date.now(),
  });
};

/**
 * The picture turned a quarter clockwise, as a PNG under the same name (and with its time, so a
 * screenshot keeps its place among the others).
 */
export const rotatePicture = async (file: File): Promise<File> => {
  const bitmap = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = bitmap.height;
  canvas.height = bitmap.width;
  const context = canvas.getContext('2d');
  context?.translate(canvas.width, 0);
  context?.rotate(Math.PI / 2);
  context?.drawImage(bitmap, 0, 0);
  bitmap.close();
  const blob = await new Promise<Blob>((resolve, reject) =>
    canvas.toBlob(
      (made) => (made ? resolve(made) : reject(new Error('The picture could not be turned'))),
      'image/png',
    ),
  );
  return new File([blob], file.name.replace(/\.[^.]+$/, '') + '.png', {
    type: 'image/png',
    lastModified: file.lastModified,
  });
};

/**
 * A copy of a picked file held in memory. Files picked in the WebView point at the picked
 * document, and on Android that reference can stop reading later in the session ("The requested
 * file could not be read…", after a model download or a while in the background); reading each
 * one at once keeps the bytes whatever happens to it.
 */
export const keepInMemory = async (file: File): Promise<File> =>
  new File([await file.arrayBuffer()], file.name, {
    type: file.type,
    lastModified: file.lastModified,
  });
