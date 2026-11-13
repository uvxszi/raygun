import type { Audio } from './effectParams';

/**
 * Smart audio on desktop: the video's `<audio>` through Web Audio (the same chain as Android's
 * `SmartAudioDsp` in `PlayerEffectsAudio.kt`): clearer dialogue (the middle's voice band lifted,
 * the sides lowered), night mode (a compressor), even loudness and scene leveling (one gain,
 * from the K-weighted loudness after the compressor), a limiter at −1 dBFS, then the volume (past
 * 100 %, its boost).
 */

/** The loudness even loudness and scene leveling aim for, LUFS (Android's too). */
export const TARGET_LUFS = -18;
export const NORMALIZE_RANGE_DB = 12;
export const LEVEL_RANGE_DB = 8;
/** How much of the gap to the target leveling closes. */
export const LEVEL_SHARE = 0.7;
/** Below this short-term loudness a scene counts as silence: leveling holds rather than lifts it. */
export const LEVEL_GATE_LUFS = -50;

const DIALOGUE_BOOST_DB = 5;
const DIALOGUE_FREQUENCY = 2200;
const DIALOGUE_SIDE_GAIN = 0.7;
/** Web Audio's compressor adds its own make-up gain (about 11 dB at these settings): 3 dB of it
 * comes off, for Android's +8 dB. */
const NIGHT = { threshold: -30, knee: 10, ratio: 3.5, attack: 0.005, release: 0.25, trimDb: -3 };

export const dbToGain = (db: number) => 10 ** (db / 20);
export const lufs = (power: number) => -0.691 + 10 * Math.log10(Math.max(power, 1e-12));

/** `current` moved toward `wanted` by at most `step`. */
export const approach = (current: number, wanted: number, step: number) =>
  wanted > current ? Math.min(wanted, current + step) : Math.max(wanted, current - step);

/**
 * BS.1770's integrated loudness over everything heard, in bounded memory: 400 ms blocks counted
 * in 0.1 LU bins (with their power), gated at −70 LUFS and 10 LU under the mean.
 */
export class LoudnessHistogram {
  private static readonly FLOOR = -70;
  private static readonly BINS = 800;
  private readonly counts = new Array<number>(LoudnessHistogram.BINS).fill(0);
  private readonly powers = new Array<number>(LoudnessHistogram.BINS).fill(0);
  private blocks = 0;

  /** Seconds of audible sound counted (blocks come 100 ms apart). */
  get seconds() {
    return this.blocks / 10;
  }

  add(power: number) {
    const loudness = lufs(power);
    if (loudness <= LoudnessHistogram.FLOOR) return;
    const bin = Math.min(
      LoudnessHistogram.BINS - 1,
      Math.max(0, Math.floor((loudness - LoudnessHistogram.FLOOR) * 10)),
    );
    this.counts[bin] = (this.counts[bin] ?? 0) + 1;
    this.powers[bin] = (this.powers[bin] ?? 0) + power;
    this.blocks += 1;
  }

  integrated(): number | null {
    if (this.blocks === 0) return null;
    const total = this.powers.reduce((sum, power) => sum + power, 0);
    const gate = lufs(total / this.blocks) - 10;
    let kept = 0;
    let count = 0;
    this.powers.forEach((power, bin) => {
      if (LoudnessHistogram.FLOOR + bin / 10 >= gate) {
        kept += power;
        count += this.counts[bin] ?? 0;
      }
    });
    return count === 0 ? lufs(total / this.blocks) : lufs(kept / count);
  }

  clear() {
    this.counts.fill(0);
    this.powers.fill(0);
    this.blocks = 0;
  }
}

/**
 * Even loudness's and scene leveling's gains from the sound's loudness, step by step (every
 * 100 ms, the K-weighted mean square of that step): the same rules as Android's.
 */
export class LoudnessFollower {
  normalizeDb = 0;
  levelDb = 0;
  shortTerm: number | null = null;
  private readonly recent: number[] = [];
  private readonly history = new LoudnessHistogram();

  get integrated() {
    return this.history.integrated();
  }

  /** Another video: measured afresh. */
  newProgram() {
    this.recent.length = 0;
    this.history.clear();
    this.shortTerm = null;
  }

  step(power: number, audio: Audio) {
    this.recent.push(power);
    if (this.recent.length > 30) this.recent.shift();
    if (this.recent.length >= 4) {
      const block = this.recent.slice(-4).reduce((sum, value) => sum + value, 0) / 4;
      this.history.add(block);
    }
    const shortTerm =
      this.recent.length >= 10
        ? lufs(this.recent.reduce((sum, value) => sum + value, 0) / this.recent.length)
        : null;
    this.shortTerm = shortTerm;
    if (audio.normalize) {
      const loudness = this.history.integrated();
      if (loudness !== null && this.history.seconds >= 3) {
        const wanted = Math.max(
          -NORMALIZE_RANGE_DB,
          Math.min(NORMALIZE_RANGE_DB, TARGET_LUFS - loudness),
        );
        // Quick at first, then slow: a video's level settles, a scene's doesn't move it.
        const perSecond = this.history.seconds < 15 ? 3 : 0.75;
        this.normalizeDb = approach(this.normalizeDb, wanted, perSecond / 10);
      }
    } else {
      this.normalizeDb = approach(this.normalizeDb, 0, 0.3);
    }
    if (audio.leveling) {
      if (shortTerm !== null && shortTerm > LEVEL_GATE_LUFS) {
        const heard = shortTerm + this.normalizeDb;
        const wanted = Math.max(
          -LEVEL_RANGE_DB,
          Math.min(LEVEL_RANGE_DB, (TARGET_LUFS - heard) * LEVEL_SHARE),
        );
        // Down quickly (a loud scene), up slowly (no pumping up of quiet ones).
        this.levelDb = approach(this.levelDb, wanted, wanted < this.levelDb ? 0.4 : 0.15);
      }
    } else {
      this.levelDb = approach(this.levelDb, 0, 0.3);
    }
  }
}

/**
 * The graph on one video element. `createMediaElementSource` can be called once per element, so
 * it's made once (for smart audio or a volume past 100 %) and stays until the video closes. It is
 * connected only once the context runs: a context still asleep would silence the video.
 */
export class VideoAudioGraph {
  private readonly context: AudioContext;
  private connected = false;
  private audio: Audio = { dialogue: false, night: false, normalize: false, leveling: false };
  private volume = 1;
  private readonly follower = new LoudnessFollower();
  private timer: ReturnType<typeof setInterval> | null = null;
  private nodes: {
    presence: BiquadFilterNode;
    side: GainNode;
    compressor: DynamicsCompressorNode;
    trim: GainNode;
    level: GainNode;
    output: GainNode;
    analysers: AnalyserNode[];
  } | null = null;
  private readonly buffer = new Float32Array(8192);

  constructor(private readonly media: HTMLMediaElement) {
    this.context = new AudioContext();
    if (this.context.state === 'running') this.connect();
    else
      void this.context.resume().then(
        () => this.connect(),
        () => undefined,
      );
  }

  setAudio(audio: Audio) {
    const measuring =
      (audio.normalize || audio.leveling) !== (this.audio.normalize || this.audio.leveling);
    this.audio = audio;
    this.apply();
    if (measuring) this.watch();
  }

  /**
   * The volume (0–2). Once connected the element plays at full and the graph's last gain takes
   * the volume: loudness is measured before it, so turning the volume down isn't undone.
   */
  setVolume(volume: number) {
    this.volume = volume;
    if (this.nodes) {
      this.media.volume = 1;
      this.nodes.output.gain.value = volume;
    } else {
      this.media.volume = Math.min(1, volume);
    }
  }

  close() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    void this.context.close().catch(() => undefined);
  }

  private connect() {
    if (this.connected || this.context.state !== 'running') return;
    this.connected = true;
    const { context } = this;
    const source = context.createMediaElementSource(this.media);
    // Two channels, mono copied to both and surround mixed down, for the middle and the sides.
    const stereo = context.createGain();
    stereo.channelCount = 2;
    stereo.channelCountMode = 'explicit';
    stereo.channelInterpretation = 'speakers';
    const splitter = context.createChannelSplitter(2);
    const mid = context.createGain();
    const sideSum = context.createGain();
    source.connect(stereo).connect(splitter);
    // Middle = (left + right) / 2, side = (left − right) / 2.
    [0.5, 0.5].forEach((weight, channel) => {
      const gain = context.createGain();
      gain.gain.value = weight;
      splitter.connect(gain, channel);
      gain.connect(mid);
    });
    [0.5, -0.5].forEach((weight, channel) => {
      const gain = context.createGain();
      gain.gain.value = weight;
      splitter.connect(gain, channel);
      gain.connect(sideSum);
    });
    const presence = context.createBiquadFilter();
    presence.type = 'peaking';
    presence.frequency.value = DIALOGUE_FREQUENCY;
    presence.Q.value = 0.8;
    const side = context.createGain();
    const inverted = context.createGain();
    inverted.gain.value = -1;
    mid.connect(presence);
    sideSum.connect(side);
    const merger = context.createChannelMerger(2);
    // Left = middle + side, right = middle − side.
    presence.connect(merger, 0, 0);
    side.connect(merger, 0, 0);
    presence.connect(merger, 0, 1);
    side.connect(inverted).connect(merger, 0, 1);

    const compressor = context.createDynamicsCompressor();
    const trim = context.createGain();
    const level = context.createGain();
    const limiter = context.createDynamicsCompressor();
    limiter.threshold.value = -1;
    limiter.knee.value = 0;
    limiter.ratio.value = 20;
    limiter.attack.value = 0.002;
    limiter.release.value = 0.1;
    // The volume last, after the limiter: past 100 % it's the boost.
    const output = context.createGain();
    merger.connect(compressor).connect(trim).connect(level).connect(limiter).connect(output);
    output.connect(context.destination);

    // BS.1770's K-weighting after the compressor: a high shelf, then a high-pass, per channel.
    const shelf = context.createBiquadFilter();
    shelf.type = 'highshelf';
    shelf.frequency.value = 1682;
    shelf.gain.value = 4;
    const highpass = context.createBiquadFilter();
    highpass.type = 'highpass';
    highpass.frequency.value = 38;
    highpass.Q.value = 0.5;
    const meterSplit = context.createChannelSplitter(2);
    const analysers = [context.createAnalyser(), context.createAnalyser()];
    trim.connect(shelf).connect(highpass).connect(meterSplit);
    analysers.forEach((analyser, channel) => {
      analyser.fftSize = 8192;
      meterSplit.connect(analyser, channel);
    });
    this.nodes = { presence, side, compressor, trim, level, output, analysers };
    this.setVolume(this.volume);
    this.apply();
    this.watch();
  }

  /** The switches as the nodes' values; off, each part passes the sound through. */
  private apply() {
    const nodes = this.nodes;
    if (!nodes) return;
    const { audio } = this;
    nodes.presence.gain.value = audio.dialogue ? DIALOGUE_BOOST_DB : 0;
    nodes.side.gain.value = audio.dialogue ? DIALOGUE_SIDE_GAIN : 1;
    nodes.compressor.threshold.value = audio.night ? NIGHT.threshold : 0;
    nodes.compressor.knee.value = audio.night ? NIGHT.knee : 0;
    nodes.compressor.ratio.value = audio.night ? NIGHT.ratio : 1;
    nodes.compressor.attack.value = NIGHT.attack;
    nodes.compressor.release.value = NIGHT.release;
    nodes.trim.gain.value = audio.night ? dbToGain(NIGHT.trimDb) : 1;
  }

  /** While even loudness or leveling is on: the loudness every 100 ms, and the gain from it. */
  private watch() {
    const measuring = this.audio.normalize || this.audio.leveling;
    if (!this.nodes) return;
    if (!measuring) {
      if (this.timer) clearInterval(this.timer);
      this.timer = null;
      this.follower.normalizeDb = 0;
      this.follower.levelDb = 0;
      this.glide(0);
      return;
    }
    if (this.timer) return;
    this.timer = setInterval(() => {
      if (this.media.paused) return;
      this.follower.step(this.power(), this.audio);
      this.glide(this.follower.normalizeDb + this.follower.levelDb);
    }, 100);
  }

  private glide(db: number) {
    const level = this.nodes?.level;
    if (!level) return;
    const now = this.context.currentTime;
    level.gain.cancelScheduledValues(now);
    level.gain.setTargetAtTime(dbToGain(db), now, 0.05);
  }

  /** The K-weighted mean square of the last 100 ms, channels summed. */
  private power() {
    const analysers = this.nodes?.analysers ?? [];
    const step = Math.min(this.buffer.length, Math.round(this.context.sampleRate / 10));
    let sum = 0;
    for (const analyser of analysers) {
      analyser.getFloatTimeDomainData(this.buffer);
      let squares = 0;
      for (let index = this.buffer.length - step; index < this.buffer.length; index += 1) {
        const sample = this.buffer[index] ?? 0;
        squares += sample * sample;
      }
      sum += squares / step;
    }
    return sum;
  }
}
