import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';

/** How often a frame is read while nothing was found. */
const SCAN_EVERY_MS = 200;
/** Barcodes are read more often, as several reads of one are compared (barcode/candidates.ts). */
const BARCODE_EVERY_MS = 120;
/** Frames are read at most this wide: plenty for a QR code on another screen, and quick. */
const SCAN_WIDTH = 640;
/** Product barcodes are read from wider frames. */
const BARCODE_WIDTH = 1280;

export type ScannerState = 'starting' | 'scanning' | 'noCamera' | 'denied';

type Detector = {
  detect: (source: CanvasImageSource) => Promise<{ rawValue: string; format?: string }[]>;
};
type DetectorClass = {
  new (options: { formats: string[] }): Detector;
  getSupportedFormats?: () => Promise<string[]>;
};

/** What a scanner reads: QR codes (device links), or the barcodes on CDs, records and books. */
export type ScanKind = 'qr' | 'barcode';

/** The product barcodes asked of the browser's own reader. */
const PRODUCT_FORMATS = ['ean_13', 'upc_a', 'ean_8', 'upc_e'];

/** A code read in a frame, with its format when the browser's reader names it. */
export type ScanRead = { text: string; format?: string };

type Reader = (canvas: HTMLCanvasElement) => Promise<string | null>;
type ReadAll = (canvas: HTMLCanvasElement) => Promise<ScanRead[]>;

/** The pixels of a canvas, for the readers written in script. */
const pixelsOf = (canvas: HTMLCanvasElement) =>
  canvas
    .getContext('2d', { willReadFrequently: true })
    ?.getImageData(0, 0, canvas.width, canvas.height) ?? null;

/**
 * Every code in a frame: the browser's own reader where it has one (Android's WebView), else
 * jsQR (Apache-2.0) for QR codes and Raygun's own EAN/UPC reader (barcode/ean.ts) for product
 * barcodes (Windows' WebView2 has no barcode reader).
 */
export const makeReadAll = async (kind: ScanKind = 'qr'): Promise<ReadAll> => {
  const native = (window as unknown as { BarcodeDetector?: DetectorClass }).BarcodeDetector;
  const wanted = kind === 'qr' ? ['qr_code'] : PRODUCT_FORMATS;
  if (native) {
    try {
      const supported = (await native.getSupportedFormats?.()) ?? ['qr_code'];
      const formats = wanted.filter((format) => supported.includes(format));
      if (formats.length > 0) {
        const detector = new native({ formats });
        return async (canvas) =>
          (await detector.detect(canvas)).map(({ rawValue, format }) => ({
            text: rawValue,
            format,
          }));
      }
    } catch {
      // Falls back to the readers in script.
    }
  }
  if (kind === 'barcode') {
    const { decodeImage } = await import('../barcode/ean');
    return async (canvas) => {
      const image = pixelsOf(canvas);
      const text = image ? decodeImage(image.data, image.width, image.height) : null;
      return text ? [{ text }] : [];
    };
  }
  const { default: jsQR } = await import('jsqr');
  return async (canvas) => {
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) return [];
    const image = context.getImageData(0, 0, canvas.width, canvas.height);
    const text = jsQR(image.data, image.width, image.height, {
      inversionAttempts: 'dontInvert',
    })?.data;
    return text ? [{ text }] : [];
  };
};

/** The first code in a frame, or null (see `makeReadAll`). */
export const makeReader = async (kind: ScanKind = 'qr'): Promise<Reader> => {
  const readAll = await makeReadAll(kind);
  return async (canvas) => (await readAll(canvas))[0]?.text ?? null;
};

/**
 * One camera session: shows the camera in `element` and reads a frame every so often until the
 * returned stop is called. Outside React, since it lives across renders.
 */
const runScanner = (
  element: () => HTMLVideoElement | null,
  onState: (state: ScannerState) => void,
  onFound: (read: ScanRead) => void,
  kind: ScanKind,
) => {
  let stopped = false;
  let stream: MediaStream | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const canvas = document.createElement('canvas');

  const start = async () => {
    onState('starting');
    if (!navigator.mediaDevices?.getUserMedia) {
      onState('noCamera');
      return;
    }
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment' },
        audio: false,
      });
    } catch (error) {
      const name = error instanceof DOMException ? error.name : '';
      onState(name === 'NotAllowedError' || name === 'SecurityError' ? 'denied' : 'noCamera');
      return;
    }
    const video = element();
    if (stopped || !video) {
      stream.getTracks().forEach((track) => track.stop());
      return;
    }
    video.srcObject = stream;
    await video.play().catch(() => undefined);
    const readAll = await makeReadAll(kind);
    onState('scanning');
    const tick = async () => {
      if (stopped) return;
      if (video.videoWidth > 0) {
        // Barcodes' thin bars need more of the camera's width than a QR code.
        const scale = Math.min(1, (kind === 'qr' ? SCAN_WIDTH : BARCODE_WIDTH) / video.videoWidth);
        canvas.width = Math.round(video.videoWidth * scale);
        canvas.height = Math.round(video.videoHeight * scale);
        canvas.getContext('2d')?.drawImage(video, 0, 0, canvas.width, canvas.height);
        const reads = await readAll(canvas).catch(() => []);
        if (!stopped) reads.forEach(onFound);
      }
      timer = setTimeout(() => void tick(), kind === 'qr' ? SCAN_EVERY_MS : BARCODE_EVERY_MS);
    };
    void tick();
  };
  void start();

  return () => {
    stopped = true;
    clearTimeout(timer);
    stream?.getTracks().forEach((track) => track.stop());
  };
};

/**
 * Reads codes from the camera into `video` while `enabled`: QR codes, or with `kind` `barcode`
 * the EAN/UPC barcodes on CDs, records and books. `onFound` gets each code's text, and its
 * format when known (the caller decides which it wants). The camera stops when disabled or unmounted.
 */
export const useQrScanner = (
  video: RefObject<HTMLVideoElement | null>,
  enabled: boolean,
  onFound: (text: string, format?: string) => void,
  kind: ScanKind = 'qr',
): ScannerState => {
  const [state, setState] = useState<ScannerState>('starting');
  const found = useRef(onFound);
  useEffect(() => {
    found.current = onFound;
  });

  useEffect(() => {
    if (!enabled) return;
    return runScanner(
      () => video.current,
      setState,
      (read) => found.current(read.text, read.format),
      kind,
    );
  }, [enabled, video, kind]);

  return state;
};
