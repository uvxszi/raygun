import { makeReadAll } from '../devices/qrScanner';
import type { ScanRead } from '../devices/qrScanner';

/**
 * Pictures are read at these widths at most (a phone photo's bars stay several pixels wide at the
 * largest); each size can read a code the others miss or misread, and the reads are compared like
 * the camera's frames (barcode/candidates.ts).
 */
const WIDTHS = [1600, 1100, 700];

/** Every barcode read in a picture (a photo of a CD's back, a book's cover), at a few sizes. */
export const readPicture = async (file: Blob): Promise<ScanRead[]> => {
  const bitmap = await createImageBitmap(file);
  const readAll = await makeReadAll('barcode');
  const reads: ScanRead[] = [];
  const done = new Set<number>();
  for (const width of WIDTHS) {
    const scale = Math.min(1, width / bitmap.width);
    const canvasWidth = Math.max(1, Math.round(bitmap.width * scale));
    // A small picture is read once, at its own size.
    if (done.has(canvasWidth)) continue;
    done.add(canvasWidth);
    const canvas = document.createElement('canvas');
    canvas.width = canvasWidth;
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    reads.push(...(await readAll(canvas).catch(() => [])));
  }
  bitmap.close();
  return reads;
};
