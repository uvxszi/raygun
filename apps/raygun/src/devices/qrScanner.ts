import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';

/** How often a frame is read while nothing was found. */
const SCAN_EVERY_MS = 200;
/** Frames are read at most this wide: plenty for a QR code on another screen, and quick. */
const SCAN_WIDTH = 640;
/** Product barcodes are read from wider frames. */
const BARCODE_WIDTH = 1280;

export type ScannerState = 'starting' | 'scanning' | 'noCamera' | 'denied';

type Detector = { detect: (source: CanvasImageSource) => Promise<{ rawValue: string }[]> };
type DetectorClass = {
  new (options: { formats: string[] }): Detector;
  getSupportedFormats?: () => Promise<string[]>;
};

/** What a scanner reads: QR codes (device links), or the barcodes on CDs, records and books. */
export type ScanKind = 'qr' | 'barcode';

/** The product barcodes asked of the browser's own reader. */
const PRODUCT_FORMATS = ['ean_13', 'upc_a', 'ean_8', 'upc_e'];

type Reader = (canvas: HTMLCanvasElement) => Promise<string | null>;

/** The pixels of a canvas, for the readers written in script. */
const pixelsOf = (canvas: HTMLCanvasElement) =>
  canvas
    .getContext('2d', { willReadFrequently: true })
    ?.getImageData(0, 0, canvas.width, canvas.height) ?? null;

/**
 * The browser's own reader where it has one (Android's WebView), else jsQR (Apache-2.0) for QR
 * codes and Raygun's own EAN/UPC reader (arcode/ean.ts) for product barcodes (Windows'
 * WebView2 has no barcode reader).
 */
export const makeReader = async (kind: ScanKind = 'qr'): Promise<Reader> => {
  const native = (window as unknown as { BarcodeDetector?: DetectorClass }).BarcodeDetector;
  const wanted = kind === 'qr' ? ['qr_code'] : PRODUCT_FORMATS;
  if (native) {
    try {
      const supported = (await native.getSupportedFormats?.()) ?? ['qr_code'];
      const formats = wanted.filter((format) => supported.includes(format));
      if (formats.length > 0) {
        const detector = new native({ formats });
        return async (canvas) => (await detector.detect(canvas))[0]?.rawValue ?? null;
      }
    } catch {
      // Falls back to the readers in script.
    }
  }
  if (kind === 'barcode') {
    const { decodeImage } = await import('../barcode/ean');
    return async (canvas) => {
      const image = pixelsOf(canvas);
      return image ? decodeImage(image.data, image.width, image.height) : null;
    };
  }
  const { default: jsQR } = await import('jsqr');
  return async (canvas) => {
    const context = canvas.getContext('2d', { willReadFrequently: true });
    if (!context) return null;
    const image = context.getImageData(0, 0, canvas.width, canvas.height);
    return (
      jsQR(image.data, image.width, image.height, { inversionAttempts: 'dontInvert' })?.data ?? null
    );
  };
};

/**
 * One camera session: shows the camera in `element` and reads a frame every so often until the
 * returned stop is called. Outside React, since it lives across renders.
 */
const runScanner = (
  element: () => HTMLVideoElement | null,
  onState: (state: ScannerState) => void,
  onFound: (text: string) => void,
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
    const read = await makeReader(kind);
    onState('scanning');
    const tick = async () => {
      if (stopped) return;
      if (video.videoWidth > 0) {
        // Barcodes' thin bars need more of the camera's width than a QR code.
        const scale = Math.min(1, (kind === 'qr' ? SCAN_WIDTH : BARCODE_WIDTH) / video.videoWidth);
        canvas.width = Math.round(video.videoWidth * scale);
        canvas.height = Math.round(video.videoHeight * scale);
        canvas.getContext('2d')?.drawImage(video, 0, 0, canvas.width, canvas.height);
        const text = await read(canvas).catch(() => null);
        if (text && !stopped) onFound(text);
      }
      timer = setTimeout(() => void tick(), SCAN_EVERY_MS);
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
 * the EAN/UPC barcodes on CDs, records and books. `onFound` gets each code's text (the caller
 * decides which it wants). The camera stops when disabled or unmounted.
 */
export const useQrScanner = (
  video: RefObject<HTMLVideoElement | null>,
  enabled: boolean,
  onFound: (text: string) => void,
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
      (text) => found.current(text),
      kind,
    );
  }, [enabled, video, kind]);

  return state;
};
