import type { AudioOffer, RememberedAudio } from './protocol';
import { languageCode } from './trackPicking';

/**
 * Sound played beside the picture (the desktop half of `video_session::audio`): another source's
 * stream of the episode, a file, or the video's own stream again when the sound is delayed. The
 * webview plays it in a second media element kept in step with the video: its place follows
 * the picture's, minus the delay; a small drift is caught up by playing a little faster or
 * slower, a large one by a jump.
 */

/** What plays as the sound: an address (the proxy's link for a stream, a blob for a file). */
export type SoundSource = {
  url: string;
  format: string | null;
  /** The offer it came from (another source's stream). */
  offer?: AudioOffer;
  /** A file picked here, by its name. */
  file?: string;
  /** The video's own stream, played again to move its sound against the picture. */
  own?: boolean;
};

/** A drift this large (seconds) is closed by a jump. */
export const JUMP_DRIFT_S = 0.3;
/** Below this the sound is in step (seconds). */
export const FINE_DRIFT_S = 0.04;
/** How much faster or slower the sound plays to close a small drift. */
const CATCH_UP = 0.04;
/** The largest delay either way (ten minutes), as the Android player keeps it. */
export const MAX_AUDIO_DELAY_MS = 600_000;

/** Where the sound should be (seconds) for the picture at `videoSeconds`; positive delays play it
 * later. */
export const soundTime = (videoSeconds: number, delayMs: number) => videoSeconds - delayMs / 1000;

export type Catchup = { jumpTo: number } | { rate: number };

/**
 * How the sound at `soundSeconds` gets back in step with `targetSeconds`, at the picture's
 * `rate`: a jump when far off, a pace a little faster or slower when close, the picture's own
 * pace once in step.
 */
export const catchUp = (soundSeconds: number, targetSeconds: number, rate: number): Catchup => {
  const ahead = soundSeconds - targetSeconds;
  if (Math.abs(ahead) > JUMP_DRIFT_S) return { jumpTo: Math.max(0, targetSeconds) };
  if (Math.abs(ahead) > FINE_DRIFT_S)
    return { rate: rate * (ahead > 0 ? 1 - CATCH_UP : 1 + CATCH_UP) };
  return { rate };
};

/** The offer that brings back the sound a title remembers: the same source in the same
 * language (one naming none, when none was named). */
export const rememberedOffer = (
  offers: readonly AudioOffer[],
  remembered: RememberedAudio | null | undefined,
): AudioOffer | null => {
  if (!remembered) return null;
  const wanted = remembered.language ? languageCode(remembered.language) : null;
  return (
    offers.find(
      (offer) =>
        offer.source === remembered.source &&
        (wanted === null
          ? offer.languages.length === 0
          : offer.languages.some((language) => languageCode(language) === wanted)),
    ) ?? null
  );
};

/** What a title remembers of an offer: its source and its first language. */
export const rememberOf = (offer: AudioOffer): RememberedAudio => ({
  source: offer.source,
  language: offer.languages[0] ?? null,
});
