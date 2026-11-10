/**
 * Pictures this run has already shown (M46.37: covers popping in again when a Library tab is
 * switched back to). The webview has them in memory, so they're loaded at once and decoded before
 * the frame is drawn instead of lazily, which left an empty tile for a moment.
 */
const shown = new Set<string>();

/** At most this many addresses are remembered; the oldest go first. */
const KEPT = 2000;

/**
 * `<img>` loading attributes for `url`, and the `onLoad` that remembers it. `eager` loads it now
 * even the first time (a picture whose place decided it's about to be seen).
 */
export const imageLoading = (url: string, eager = false) => {
  const known = shown.has(url);
  return {
    loading: known || eager ? ('eager' as const) : ('lazy' as const),
    decoding: known ? ('sync' as const) : ('async' as const),
    onLoad: () => {
      if (shown.has(url)) return;
      if (shown.size >= KEPT) {
        const oldest = shown.values().next().value;
        if (oldest !== undefined) shown.delete(oldest);
      }
      shown.add(url);
    },
  };
};
