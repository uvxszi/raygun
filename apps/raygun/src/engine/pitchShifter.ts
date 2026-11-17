/**
 * Pitch without tempo on desktop (`SoundPrefs::pitch`, a video's `VideoSound::pitch`): Web Audio
 * has no pitch shifter, so an AudioWorklet runs the classic two-tap delay-line one. Each channel
 * is written into a short ring; two read taps half a window apart drift through the delay at
 * (1 − ratio) samples a sample (reading faster or slower than it's written moves the pitch) and
 * wrap round the window, each faded in and out on an equal-power curve so the wraps don't click.
 * Android's player does the same with Media3's Sonic (`PlaybackParameters` pitch).
 */

/** The longest shift either way, in tenths of a semitone (an octave). */
export const PITCH_LIMIT = 120;

/** The window the taps drift through, ms: long enough for low notes, short enough not to echo. */
export const WINDOW_MS = 50;

/** A shift in tenths of a semitone as a frequency ratio. */
export const pitchRatio = (tenths: number) =>
  2 ** (Math.max(-PITCH_LIMIT, Math.min(PITCH_LIMIT, tenths)) / 120);

/**
 * How far the first tap's phase (its delay as a share of the window) moves each sample: a higher
 * pitch reads faster than it's written, so the delay shrinks.
 */
export const phaseStep = (ratio: number, windowSamples: number) => (1 - ratio) / windowSamples;

/** `phase` kept within [0, 1). */
export const wrapPhase = (phase: number) => phase - Math.floor(phase);

/**
 * The two taps' gains at the first tap's `phase` (the second is half a window on): sin and cos of
 * π·phase, so their powers add up to one and each is silent where it wraps.
 */
export const tapGains = (phase: number): [number, number] => [
  Math.sin(Math.PI * phase),
  Math.abs(Math.cos(Math.PI * phase)),
];

/** The worklet's source: the same steps as above, per channel, with a linear tap between samples. */
const WORKLET_SOURCE = `
class RaygunPitchShift extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [{ name: 'ratio', defaultValue: 1, minValue: 0.5, maxValue: 2, automationRate: 'k-rate' }];
  }
  constructor() {
    super();
    this.window = Math.max(64, Math.round(sampleRate * ${WINDOW_MS} / 1000));
    this.size = this.window + 4;
    this.rings = [];
    this.write = 0;
    this.phase = 0;
  }
  tap(ring, write, delay) {
    let at = write - delay;
    while (at < 0) at += this.size;
    const low = Math.floor(at);
    const part = at - low;
    const next = low + 1 >= this.size ? 0 : low + 1;
    return ring[low] * (1 - part) + ring[next] * part;
  }
  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];
    const ratio = parameters.ratio[0];
    const step = (1 - ratio) / this.window;
    const frames = output[0] ? output[0].length : 0;
    while (this.rings.length < output.length) this.rings.push(new Float32Array(this.size));
    let phase = this.phase;
    let write = this.write;
    for (let channel = 0; channel < output.length; channel += 1) {
      const source = input && input[channel] ? input[channel] : null;
      const target = output[channel];
      const ring = this.rings[channel];
      phase = this.phase;
      write = this.write;
      for (let index = 0; index < frames; index += 1) {
        ring[write] = source ? source[index] : 0;
        const second = phase + 0.5 >= 1 ? phase - 0.5 : phase + 0.5;
        const first = this.tap(ring, write, phase * this.window);
        const other = this.tap(ring, write, second * this.window);
        target[index] = first * Math.sin(Math.PI * phase) + other * Math.abs(Math.cos(Math.PI * phase));
        phase += step;
        phase -= Math.floor(phase);
        write = write + 1 >= this.size ? 0 : write + 1;
      }
    }
    this.phase = phase;
    this.write = write;
    return true;
  }
}
registerProcessor('raygun-pitch-shift', RaygunPitchShift);
`;

/** The module loaded once per context: whether it could be. */
const loaded = new WeakMap<BaseAudioContext, Promise<boolean>>();

const loadModule = (context: BaseAudioContext): Promise<boolean> => {
  const known = loaded.get(context);
  if (known) return known;
  const worklet = (context as { audioWorklet?: AudioWorklet }).audioWorklet;
  if (!worklet || typeof URL.createObjectURL !== 'function' || typeof Blob === 'undefined') {
    const none = Promise.resolve(false);
    loaded.set(context, none);
    return none;
  }
  const url = URL.createObjectURL(new Blob([WORKLET_SOURCE], { type: 'text/javascript' }));
  const loading = worklet.addModule(url).then(
    () => true,
    () => false,
  );
  loaded.set(context, loading);
  return loading;
};

/**
 * A place in a Web Audio chain for the pitch shifter: `input` goes straight to `output` until a
 * shift is asked for, then through the worklet (loaded the first time). Without AudioWorklet the
 * sound passes as it is.
 */
export class PitchStage {
  readonly input: GainNode;
  readonly output: GainNode;
  private node: AudioWorkletNode | null = null;
  private loading = false;
  private ratio = 1;
  private through = false;

  constructor(private readonly context: BaseAudioContext) {
    this.input = context.createGain();
    this.output = context.createGain();
    this.input.connect(this.output);
  }

  /** The shift in tenths of a semitone; 0 takes the worklet out of the chain. */
  setPitch(tenths: number) {
    this.ratio = pitchRatio(tenths);
    this.update();
  }

  private update() {
    const shifting = this.ratio !== 1;
    if (shifting && !this.node) {
      this.load();
      return;
    }
    const node = this.node;
    if (!node) return;
    node.parameters.get('ratio')?.setValueAtTime(this.ratio, this.context.currentTime);
    if (shifting === this.through) return;
    this.input.disconnect();
    node.disconnect();
    if (shifting) this.input.connect(node).connect(this.output);
    else this.input.connect(this.output);
    this.through = shifting;
  }

  private load() {
    if (this.loading) return;
    this.loading = true;
    void loadModule(this.context).then((ready) => {
      if (!ready) return;
      try {
        this.node = new AudioWorkletNode(this.context, 'raygun-pitch-shift', {
          outputChannelCount: [2],
        });
      } catch {
        return;
      }
      this.update();
    });
  }
}
