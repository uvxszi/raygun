import { commands, events } from '@raygun/bindings';
import { useEffect } from 'react';

type SpeechState = 'speaking' | 'range' | 'ended' | 'error';

const report = (state: SpeechState, id: string, extra?: { start: number; end: number }) =>
  void commands.ttsReport(JSON.stringify({ state, id, ...extra }));

/** 16-bit little-endian mono PCM (base64) as samples between −1 and 1. */
export const pcmSamples = (pcm: string) => {
  const bytes = atob(pcm);
  const view = new DataView(new ArrayBuffer(bytes.length));
  for (let index = 0; index < bytes.length; index += 1)
    view.setUint8(index, bytes.charCodeAt(index));
  const samples = new Float32Array(Math.floor(bytes.length / 2));
  for (let index = 0; index < samples.length; index += 1) {
    samples[index] = view.getInt16(index * 2, true) / 32768;
  }
  return samples;
};

type Clip = { id: string; buffer: AudioBuffer; last: boolean };

/**
 * Plays what downloaded voices made, one clip after another, pausing and going on where it was
 * (the audio context is suspended and resumed), at a volume that changes at once. Reports
 * `speaking` as each clip starts and `ended` after the last.
 */
const clipPlayer = () => {
  let context: AudioContext | null = null;
  let gain: GainNode | null = null;
  let volume = 1;
  let queue: Clip[] = [];
  let current: AudioBufferSourceNode | null = null;
  // A stop makes the clips of before it stale.
  let generation = 0;

  const playNext = () => {
    if (current || !context) return;
    const next = queue.shift();
    if (!next) return;
    const source = context.createBufferSource();
    source.buffer = next.buffer;
    source.connect(gain ?? context.destination);
    const mine = generation;
    source.onended = () => {
      if (mine !== generation) return;
      current = null;
      if (next.last) report('ended', next.id);
      playNext();
    };
    current = source;
    source.start();
    report('speaking', next.id);
  };

  return {
    add: (id: string, sampleRate: number, pcm: string, last: boolean) => {
      if (!context) {
        context = new AudioContext();
        gain = context.createGain?.() ?? null;
        if (gain) {
          gain.gain.value = volume;
          gain.connect(context.destination);
        }
        void context.resume();
      }
      const samples = pcmSamples(pcm);
      const buffer = context.createBuffer(1, Math.max(samples.length, 1), sampleRate);
      buffer.copyToChannel(samples, 0);
      queue.push({ id, buffer, last });
      playNext();
    },
    pause: () => void context?.suspend(),
    resume: () => void context?.resume(),
    setVolume: (level: number) => {
      volume = Math.min(1, Math.max(0, level));
      if (gain) gain.gain.value = volume;
    },
    stop: () => {
      generation += 1;
      queue = [];
      const playing = current;
      current = null;
      if (playing) {
        playing.onended = null;
        try {
          playing.stop();
        } catch {
          // Already stopped.
        }
      }
      void context?.resume();
    },
  };
};

/** A system voice for `language` (a BCP 47 tag), when there is one. */
const voiceFor = (synth: SpeechSynthesis, language: string) => {
  const wanted = language.toLowerCase();
  const voices = synth.getVoices?.() ?? [];
  return (
    voices.find((voice) => voice.lang.toLowerCase() === wanted) ??
    voices.find((voice) => voice.lang.toLowerCase().split(/[-_]/)[0] === wanted.split(/[-_]/)[0])
  );
};

/**
 * Read-aloud on desktop (M46.6): the webview's `speechSynthesis` with the system's voices, or the
 * speech downloaded voices made. Rust splits the text into sentences and sends
 * `speechCommandEvent`s; this speaks or plays them in order and reports back in the Android
 * engine's JSON: `speaking` as each sentence starts, `range` as each word starts (system voices),
 * `ended` after the last. Mount once, in the app shell; on Android nothing is ever sent to it.
 */
export const useSpeechHost = () => {
  useEffect(() => {
    const synth = typeof window.speechSynthesis === 'undefined' ? null : window.speechSynthesis;
    void commands.ttsSetAvailable(synth !== null);
    const clips = clipPlayer();
    // A newer command makes callbacks of the cancelled utterances stale.
    let generation = 0;

    const unlisten = events.speechCommandEvent.listen(({ payload }) => {
      switch (payload.type) {
        case 'clip':
          clips.add(payload.id, payload.sampleRate, payload.pcm, payload.last);
          return;
        case 'pause':
          clips.pause();
          return;
        case 'resume':
          clips.resume();
          return;
        case 'volume':
          clips.setVolume(payload.volume ?? 1);
          return;
        case 'stop':
          generation += 1;
          synth?.cancel();
          clips.stop();
          return;
        default:
          break;
      }
      generation += 1;
      const current = generation;
      clips.stop();
      if (!synth) return;
      synth.cancel();
      const book = payload.language ? voiceFor(synth, payload.language) : undefined;
      for (const item of payload.items) {
        const last = item.id.endsWith(':end');
        const utterance = new SpeechSynthesisUtterance(item.text);
        utterance.rate = payload.rate ?? 1;
        utterance.pitch = payload.pitch ?? 1;
        utterance.volume = payload.volume ?? 1;
        // A passage in another language is read by a voice of its own, when there is one.
        const voice = (item.language ? voiceFor(synth, item.language) : undefined) ?? book;
        if (voice) {
          utterance.voice = voice;
          utterance.lang = voice.lang;
        }
        utterance.onstart = () => {
          if (current === generation) report('speaking', item.id);
        };
        utterance.onboundary = (event) => {
          if (current !== generation || event.name !== 'word') return;
          report('range', item.id, {
            start: event.charIndex,
            end: event.charIndex + (event.charLength ?? 0),
          });
        };
        utterance.onend = () => {
          if (current === generation && last) report('ended', item.id);
        };
        utterance.onerror = (event) => {
          // Cancelling (a new command, Stop) isn't a failure.
          if (current !== generation || event.error === 'interrupted' || event.error === 'canceled')
            return;
          report('error', item.id);
        };
        synth.speak(utterance);
      }
    });
    return () => {
      generation += 1;
      synth?.cancel();
      clips.stop();
      void unlisten.then((stop) => stop());
    };
  }, []);
};
