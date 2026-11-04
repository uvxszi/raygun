import type { Reverb, SoundPrefs } from '@raygun/bindings';

import { withSoundDefaults } from '../sound/soundPrefs';
import type { FullSound } from '../sound/soundPrefs';

/** Centre frequencies of the equalizer's bands, Hz; the same as `EQ_FREQUENCIES` in Rust. */
export const EQ_FREQUENCIES = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

/** One deck's sound: its own level and fades, and a meter for its loudness. */
export type DeckAudio = {
  /** The song's own gain in dB (its volume, normalization, live levelling); `smoothMs` glides. */
  setGain: (db: number, smoothMs?: number) => void;
  /** Sets the fade level (0–1) at once. */
  setFade: (level: number) => void;
  /** Glides the fade level to `level` over `ms`, on an equal-power curve. */
  fadeTo: (level: number, ms: number) => void;
  /** The K-weighted mean square of the last 100 ms, channels summed; null without a meter. */
  power: () => number | null;
};

/** Where the engine's decks play through: Web Audio's graph, or the bare elements. */
export type SoundBackend = {
  deck: (media: HTMLMediaElement) => DeckAudio;
  apply: (sound: SoundPrefs) => void;
  setVolume: (volume: number) => void;
  /** Wakes a suspended audio context (browsers start one asleep until there's been a click). */
  resume: () => void;
};

export const dbToGain = (db: number) => 10 ** (db / 20);

/** A reverb's impulse: how long it rings, how fast it fades, how much is mixed in. */
const REVERBS: Record<Exclude<Reverb, 'off'>, { seconds: number; decay: number; wet: number }> = {
  smallRoom: { seconds: 0.5, decay: 4, wet: 0.16 },
  mediumRoom: { seconds: 0.9, decay: 3.5, wet: 0.18 },
  largeRoom: { seconds: 1.4, decay: 3, wet: 0.2 },
  mediumHall: { seconds: 2.2, decay: 2.6, wet: 0.24 },
  largeHall: { seconds: 3.2, decay: 2.2, wet: 0.27 },
  plate: { seconds: 1.8, decay: 2.8, wet: 0.24 },
};

/**
 * The gain taken off before the equalizer so boosts don't clip (RiPlay's automatic headroom):
 * the largest band boost or the bass boost, whichever is more.
 */
export const headroomDb = (sound: FullSound) => {
  const bands = sound.eqEnabled ? Math.max(0, ...sound.eqBands.map((gain) => gain / 10)) : 0;
  return Math.max(bands, (sound.bassBoost / 100) * 12);
};

/** Ramps on an equal-power curve: rising like a sine, falling like a cosine. */
const fadeCurve = (from: number, to: number, points = 64) => {
  const curve = new Float32Array(points);
  for (let index = 0; index < points; index += 1) {
    const progress = index / (points - 1);
    curve[index] =
      to >= from
        ? from + (to - from) * Math.sin((progress * Math.PI) / 2)
        : to + (from - to) * Math.cos((progress * Math.PI) / 2);
  }
  return curve;
};

/**
 * Music on desktop plays through this Web Audio graph (M46.12). Each deck (an `<audio>` element)
 * has its own level and fade, so two can cross; their mix goes through the preamp, ten
 * BiquadFilters (a low shelf, peaking bands, a high shelf, as Nuclear's hifi package lays them
 * out), the bass shelf, reverb (a ConvolverNode on a generated impulse) beside the dry signal,
 * balance, mono, a limiter at −1 dBFS and the master volume. Each deck also feeds a K-weighted
 * meter (two filters and an analyser per channel) for loudness normalization.
 */
export class WebAudioSound implements SoundBackend {
  private readonly mix: GainNode;
  private readonly preamp: GainNode;
  private readonly bands: BiquadFilterNode[];
  private readonly bass: BiquadFilterNode;
  private readonly convolver: ConvolverNode;
  private readonly wet: GainNode;
  private readonly panner: StereoPannerNode;
  private readonly mono: GainNode;
  private readonly master: GainNode;
  private reverb: Reverb = 'off';

  constructor(private readonly context: AudioContext) {
    // Browsers start a context asleep until there's been a click, and only a click (or a key)
    // may wake it: the first one anywhere does, so a song started later by Rust has sound.
    const wake = () => {
      if (context.state !== 'suspended') {
        window.removeEventListener('pointerdown', wake, true);
        window.removeEventListener('keydown', wake, true);
        return;
      }
      void context.resume().catch(() => undefined);
    };
    window.addEventListener('pointerdown', wake, true);
    window.addEventListener('keydown', wake, true);
    this.mix = context.createGain();
    this.preamp = context.createGain();
    this.bands = EQ_FREQUENCIES.map((frequency, index) => {
      const filter = context.createBiquadFilter();
      const last = EQ_FREQUENCIES.length - 1;
      filter.type = index === 0 ? 'lowshelf' : index === last ? 'highshelf' : 'peaking';
      filter.frequency.value = frequency;
      if (filter.type === 'peaking') {
        const below = EQ_FREQUENCIES[index - 1] ?? frequency / 2;
        const above = EQ_FREQUENCIES[index + 1] ?? frequency * 2;
        filter.Q.value = (2 * frequency) / Math.abs(above - below);
      }
      filter.gain.value = 0;
      return filter;
    });
    this.bass = context.createBiquadFilter();
    this.bass.type = 'lowshelf';
    this.bass.frequency.value = 100;
    this.bass.gain.value = 0;
    const dry = context.createGain();
    this.convolver = context.createConvolver();
    this.wet = context.createGain();
    this.wet.gain.value = 0;
    const merge = context.createGain();
    this.panner = context.createStereoPanner();
    this.mono = context.createGain();
    const limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -1;
    limiter.knee.value = 0;
    limiter.ratio.value = 20;
    limiter.attack.value = 0.002;
    limiter.release.value = 0.15;
    this.master = context.createGain();

    let chain: AudioNode = this.mix.connect(this.preamp);
    for (const band of this.bands) chain = chain.connect(band);
    chain.connect(this.bass);
    this.bass.connect(dry).connect(merge);
    this.bass.connect(this.convolver).connect(this.wet).connect(merge);
    merge
      .connect(this.panner)
      .connect(this.mono)
      .connect(limiter)
      .connect(this.master)
      .connect(context.destination);
  }

  deck(media: HTMLMediaElement): DeckAudio {
    const { context } = this;
    const source = context.createMediaElementSource(media);
    const level = context.createGain();
    const fade = context.createGain();
    source.connect(level).connect(fade).connect(this.mix);

    // BS.1770's K-weighting: a high shelf (+4 dB above ~1.7 kHz), then a high-pass at ~38 Hz.
    const shelf = context.createBiquadFilter();
    shelf.type = 'highshelf';
    shelf.frequency.value = 1682;
    shelf.gain.value = 4;
    const highpass = context.createBiquadFilter();
    highpass.type = 'highpass';
    highpass.frequency.value = 38;
    highpass.Q.value = 0.5;
    const splitter = context.createChannelSplitter(2);
    const analysers = [context.createAnalyser(), context.createAnalyser()];
    source.connect(shelf).connect(highpass).connect(splitter);
    analysers.forEach((analyser, channel) => {
      analyser.fftSize = 8192;
      splitter.connect(analyser, channel);
    });
    const buffer = new Float32Array(8192);
    const stepSamples = Math.min(buffer.length, Math.round(context.sampleRate / 10));

    const glide = (param: AudioParam, value: number, ms: number) => {
      const now = context.currentTime;
      param.cancelScheduledValues(now);
      if (ms > 0) {
        param.setValueAtTime(param.value, now);
        param.setTargetAtTime(value, now, ms / 3000);
      } else {
        param.setValueAtTime(value, now);
      }
    };

    return {
      setGain: (db, smoothMs = 0) => glide(level.gain, dbToGain(db), smoothMs),
      setFade: (value) => glide(fade.gain, value, 0),
      fadeTo: (value, ms) => {
        const now = context.currentTime;
        const from = fade.gain.value;
        fade.gain.cancelScheduledValues(now);
        if (ms <= 0 || from === value) {
          fade.gain.setValueAtTime(value, now);
          return;
        }
        fade.gain.setValueCurveAtTime(fadeCurve(from, value), now, ms / 1000);
      },
      power: () => {
        let sum = 0;
        for (const analyser of analysers) {
          analyser.getFloatTimeDomainData(buffer);
          let squares = 0;
          for (let index = buffer.length - stepSamples; index < buffer.length; index += 1) {
            const sample = buffer[index] ?? 0;
            squares += sample * sample;
          }
          sum += squares / stepSamples;
        }
        return sum;
      },
    };
  }

  apply(prefs: SoundPrefs) {
    const sound = withSoundDefaults(prefs);
    const eq = sound.eqEnabled;
    this.bands.forEach((band, index) => {
      band.gain.value = eq ? (sound.eqBands[index] ?? 0) / 10 : 0;
    });
    const preamp = eq ? sound.preamp / 10 : 0;
    this.preamp.gain.value = dbToGain(preamp - headroomDb(sound));
    this.bass.gain.value = (sound.bassBoost / 100) * 12;
    if (sound.reverb !== this.reverb) {
      this.reverb = sound.reverb;
      this.convolver.buffer = sound.reverb === 'off' ? null : this.impulse(sound.reverb);
    }
    this.wet.gain.value = sound.reverb === 'off' ? 0 : REVERBS[sound.reverb].wet;
    this.panner.pan.value = Math.max(-1, Math.min(1, sound.balance / 100));
    // A one-channel node mixes both sides together; the speakers get it on both again.
    this.mono.channelCount = sound.mono ? 1 : 2;
    this.mono.channelCountMode = sound.mono ? 'explicit' : 'max';
  }

  setVolume(volume: number) {
    this.master.gain.value = Math.max(0, Math.min(1, volume));
  }

  resume() {
    if (this.context.state === 'suspended') void this.context.resume().catch(() => undefined);
  }

  /** Decaying stereo noise: a room's or a plate's ring. */
  private impulse(reverb: Exclude<Reverb, 'off'>) {
    const { seconds, decay } = REVERBS[reverb];
    const rate = this.context.sampleRate;
    const length = Math.max(1, Math.round(seconds * rate));
    const buffer = this.context.createBuffer(2, length, rate);
    // A plate is denser and brighter; rooms lose their highs as they ring.
    const smoothing = reverb === 'plate' ? 0.1 : 0.45;
    for (let channel = 0; channel < 2; channel += 1) {
      const data = buffer.getChannelData(channel);
      let previous = 0;
      for (let index = 0; index < length; index += 1) {
        const noise = Math.random() * 2 - 1;
        previous = previous * smoothing + noise * (1 - smoothing);
        data[index] = previous * (1 - index / length) ** decay;
      }
    }
    return buffer;
  }
}

/**
 * Without Web Audio the elements play as they are: a song's gain can only turn it down, fades
 * step the element's volume, and nothing is measured or equalized.
 */
export const elementSound = (): SoundBackend => {
  let master = 1;
  const decks: (() => void)[] = [];
  return {
    deck: (media) => {
      let gain = 1;
      let fade = 1;
      let timer: ReturnType<typeof setInterval> | null = null;
      const update = () => {
        media.volume = Math.max(0, Math.min(1, master * gain * fade));
      };
      decks.push(update);
      return {
        setGain: (db) => {
          gain = Math.min(1, dbToGain(db));
          update();
        },
        setFade: (value) => {
          if (timer) clearInterval(timer);
          fade = value;
          update();
        },
        fadeTo: (value, ms) => {
          if (timer) clearInterval(timer);
          const steps = Math.max(1, Math.round(ms / 50));
          const from = fade;
          let step = 0;
          timer = setInterval(() => {
            step += 1;
            fade = from + ((value - from) * step) / steps;
            update();
            if (step >= steps && timer) clearInterval(timer);
          }, 50);
        },
        power: () => null,
      };
    },
    apply: () => undefined,
    setVolume: (volume) => {
      master = volume;
      decks.forEach((update) => update());
    },
    resume: () => undefined,
  };
};
