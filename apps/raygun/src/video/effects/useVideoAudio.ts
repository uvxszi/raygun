import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

import { isAudioOff } from './effectParams';
import type { Audio } from './effectParams';
import { VideoAudioGraph } from './smartAudio';

/**
 * The video's sound: its volume (past 100 % through Web Audio's gain) and smart audio
 * (`VideoAudioGraph`). The graph is made the first time either needs it and stays with the
 * element; without Web Audio the video plays as it is, up to 100 %.
 */
export const useVideoAudio = (
  videoRef: RefObject<HTMLVideoElement | null>,
  volume: number,
  muted: boolean,
  audio: Audio,
) => {
  const graph = useRef<VideoAudioGraph | null>(null);
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.muted = muted;
    const wanted = volume > 1 || !isAudioOff(audio);
    if (wanted && !graph.current && typeof AudioContext !== 'undefined') {
      try {
        graph.current = new VideoAudioGraph(video);
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
  useEffect(
    () => () => {
      graph.current?.close();
    },
    [],
  );
};
