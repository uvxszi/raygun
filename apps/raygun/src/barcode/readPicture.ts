import { makeReader } from '../devices/qrScanner';

/** Pictures are read at most this wide (a phone photo's bars stay several pixels wide). */
const MAX_WIDTH = 1600;

/** The barcode in a picture (a photo of a CD's back, a book's cover), or null. */
export const readPicture = async (file: Blob): Promise<string | null> => {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, MAX_WIDTH / bitmap.width);
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const read = await makeReader('barcode');
  return read(canvas).catch(() => null);
};
