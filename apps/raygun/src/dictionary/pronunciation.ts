import { outputSink } from '../engine/outputSink';

type Sinkable = { setSinkId?: (id: string) => Promise<void> };

/** The recording playing now: a new one stops it. */
let playing: HTMLAudioElement | null = null;

/**
 * Plays a word's recording (a link through Raygun's proxy, so Offline Mode and the dictionaries'
 * User-Agent hold) on the output the app plays through. It's a second long: music isn't paused.
 */
export const playPronunciation = async (url: string) => {
  playing?.pause();
  const audio = new Audio(url);
  playing = audio;
  const sink = outputSink();
  if (sink) await (audio as Sinkable).setSinkId?.(sink).catch(() => undefined);
  await audio.play();
};

/** Stops a recording that's playing (the lookup closed). */
export const stopPronunciation = () => {
  playing?.pause();
  playing = null;
};
