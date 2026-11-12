import type { SyntheticEvent } from 'react';

/**
 * Pictures this run has already shown (M46.37: covers popping in again when a Library tab is
 * switched back to). The webview has them in memory, so they're loaded at once and decoded before
 * the frame is drawn instead of lazily, which left an empty tile for a moment; they show at once,
 * without fading in again (shell/Picture).
 */
const shown = new Set<string>();

/** At most this many addresses are remembered; the oldest go first. */
const KEPT = 2000;

/** Data Saver's "pictures only once on screen": nothing new loads ahead of being seen. */
let whenVisible = false;

export const setImagesWhenVisible = (on: boolean) => {
  whenVisible = on;
};

/** Whether `url` has loaded once this run (its picture shows on the first frame it's drawn). */
export const wasShown = (url: string) => shown.has(url);

/** Notes that `url` has loaded. */
export const rememberShown = (url: string) => {
  if (shown.has(url)) return;
  if (shown.size >= KEPT) {
    const oldest = shown.values().next().value;
    if (oldest !== undefined) shown.delete(oldest);
  }
  shown.add(url);
};

/** For tests: nothing has been shown yet. */
export const forgetShownImages = () => shown.clear();

/**
 * `<img>` loading attributes for `url`, and the `onLoad` that remembers it. `eager` loads it now
 * even the first time (a picture whose place decided it's about to be seen), unless Data Saver
 * waits for pictures to be on screen.
 */
export const imageLoading = (url: string, eager = false) => {
  const known = shown.has(url);
  return {
    loading: known || (eager && !whenVisible) ? ('eager' as const) : ('lazy' as const),
    decoding: known ? ('sync' as const) : ('async' as const),
    onLoad: () => rememberShown(url),
  };
};

/**
 * Whether pictures fade now (`data-motion` lively or calm, as styles.css fades them): Reduced and
 * Off show them at once.
 */
export const picturesFade = () => {
  const level = document.documentElement.dataset.motion;
  if (level !== 'lively' && level !== 'calm') return false;
  return !(
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
};

/**
 * Shows a loaded picture once it's decoded (`data-loaded`, which styles.css fades in from a
 * `data-fade` picture's transparent start), so it never appears half drawn; then `then`, if
 * given. One decode per picture, nothing kept waiting: a picture that can't decode shows anyway.
 */
export const revealDecoded = (image: HTMLImageElement, then?: () => void) => {
  const reveal = () => {
    image.dataset.loaded = '';
    then?.();
  };
  if (typeof image.decode === 'function') image.decode().then(reveal, reveal);
  else reveal();
};

/**
 * A plain `<img>`'s attributes that fade it in once it has loaded and decoded, for pictures sized
 * by their own shape (a book's illustrations), where shell/Picture's frame can't be: as
 * `imageLoading`, and a picture seen before this run shows at once.
 */
export const fadingImage = (url: string, eager = false) => {
  const loading = imageLoading(url, eager);
  const known = shown.has(url);
  return {
    loading: loading.loading,
    decoding: loading.decoding,
    'data-fade': known ? undefined : '',
    onLoad: (event: SyntheticEvent<HTMLImageElement>) => {
      loading.onLoad();
      if (known) event.currentTarget.dataset.loaded = '';
      else revealDecoded(event.currentTarget);
    },
  };
};
