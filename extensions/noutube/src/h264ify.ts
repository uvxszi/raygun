// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/h264ify.ts, itself adapted from https://github.com/erkserkserks/h264ify (MIT).
//
// YouTube serves H.264 once the page says it can't play VP8, VP9, AV1 or WebM: most phones decode
// H.264 in hardware, which saves battery and keeps older devices smooth.

const DISALLOWED = ['webm', 'vp8', 'vp9', 'av01'];

const allowed = (type: string | undefined) =>
  typeof type === 'string' && !DISALLOWED.some((codec) => type.includes(codec));

export function installH264ify() {
  try {
    const canPlayType = HTMLMediaElement.prototype.canPlayType;
    HTMLVideoElement.prototype.canPlayType = function (this: HTMLVideoElement, type: string) {
      return allowed(type) ? canPlayType.call(this, type) : '';
    };
  } catch (error) {
    console.warn('[YouTube enhancements] H.264:', error);
  }

  try {
    if (typeof MediaSource !== 'undefined' && typeof MediaSource.isTypeSupported === 'function') {
      const isTypeSupported = MediaSource.isTypeSupported.bind(MediaSource);
      MediaSource.isTypeSupported = (type: string) => allowed(type) && isTypeSupported(type);
    }
  } catch (error) {
    console.warn('[YouTube enhancements] H.264:', error);
  }
}
