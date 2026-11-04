/**
 * Loop this video without a gap (desktop). A file, or a stream the webview plays itself, loops in
 * the media element (`loop`): the same decoder and the browser's own cache, no `ended`. An HLS or
 * DASH stream (Media Source) keeps its start buffered and goes back to it by a seek at the end,
 * with the same source attached: nothing reloads.
 */

import type { MediaEngine } from './mediaEngine';

/** Turns looping on or off for what plays in `video` through `engine`. */
export const applyLoop = (video: HTMLVideoElement, engine: MediaEngine | null, on: boolean) => {
  video.loop = on && (engine === null || engine.kind === 'native');
  engine?.keepStart(on);
};

/** At the end of a looping Media Source stream: back to its start, still playing. */
export const loopAround = (video: HTMLVideoElement) => {
  video.currentTime = 0;
  void video.play()?.catch(() => {});
};

/** How close to the end and the start two times must be to read as one going round. */
const WRAP_MARGIN_MS = 1_500;

/** The time moved from the end back to the start: a loop went round. */
export const wentRound = (beforeMs: number, nowMs: number, durationMs: number) =>
  durationMs > WRAP_MARGIN_MS * 2 &&
  beforeMs >= durationMs - WRAP_MARGIN_MS &&
  nowMs <= WRAP_MARGIN_MS;
