import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';

import { isAudioOff } from './effectParams';
import type { Audio } from './effectParams';
import { VideoAudioGraph } from './smartAudio';

export type VideoAudio = {
  /**
   * The sound goes through the graph, the sound beside the picture's too when there is one: its
   * own element then plays at full, as the graph's last gain is the volume.
   */
  routed: boolean;
  /** The element the sound beside the picture plays in (`useExternalAudio`); null without one. */
  setExternal: (element: HTMLAudioElement | null) => void;
};

/**
 * The video's sound: its volume (past 100 % through Web Audio's gain) and smart audio
 * (`VideoAudioGraph`), with the sound beside the picture (another source's, a file, the video's
 * own moved by the delay) through the same chain. The graph is made the first time either needs
 * it and stays with the element; without Web Audio the video plays as it is, up to 100 %.
 * `muted` is the video element's (it's muted while the sound beside it plays).
 */
export const useVideoAudio = (
  videoRef: RefObject<HTMLVideoElement | null>,
  volume: number,
  muted: boolean,
  audio: Audio,
): VideoAudio => {
  const graph = useRef<VideoAudioGraph | null>(null);
  const [connected, setConnected] = useState(false);
  const [external, setExternal] = useState<HTMLAudioElement | null>(null);
  const [attached, setAttached] = useState<HTMLAudioElement | null>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.muted = muted;
    const wanted = volume > 1 || !isAudioOff(audio);
    if (wanted && !graph.current && typeof AudioContext !== 'undefined') {
      try {
        graph.current = new VideoAudioGraph(video, () => setConnected(true));
      } catch {
        // No boost or smart audio: it plays as it is.
      }
    }
    if (graph.current) {
      graph.current.setVolume(volume);
      graph.current.setAudio(audio);
    } else {
      video.volume = Math.min(1, volume);
    }
  }, [volume, muted, audio, videoRef]);

  // The sound beside the picture joins the chain once the graph runs; gone, it lets go of it.
  useEffect(() => {
    const current = graph.current;
    if (!connected || !current || !external) return;
    if (!current.attach(external)) return;
    setAttached(external);
    return () => {
      current.detach(external);
      setAttached((element) => (element === external ? null : element));
    };
  }, [connected, external]);

  useEffect(
    () => () => {
      graph.current?.close();
    },
    [],
  );

  return { routed: connected && (external === null || attached === external), setExternal };
};
