import { LoudnessMeter, meanSquare, toLufs } from './loudness';
import { EQ_FREQUENCIES, WebAudioSound, dbToGain, headroomDb } from './soundGraph';
import { SOUND_DEFAULTS } from '../sound/soundPrefs';

/** An AudioParam that remembers its value and its last scheduled curve. */
class FakeParam {
  value = 0;
  curve: Float32Array | null = null;
  cancelScheduledValues = () => this;
  setValueAtTime = (value: number) => {
    this.value = value;
    return this;
  };
  setTargetAtTime = (value: number) => {
    this.value = value;
    return this;
  };
  setValueCurveAtTime = (curve: Float32Array) => {
    this.curve = curve;
    this.value = curve[curve.length - 1] ?? this.value;
    return this;
  };
}

class FakeNode {
  readonly outputs: FakeNode[] = [];
  channelCount = 2;
  channelCountMode = 'max';
  constructor(readonly kind: string) {}
  connect(target: FakeNode) {
    this.outputs.push(target);
    return target;
  }
}

class FakeGain extends FakeNode {
  gain = new FakeParam();
  constructor() {
    super('gain');
    this.gain.value = 1;
  }
}

class FakeFilter extends FakeNode {
  type = 'lowpass';
  frequency = new FakeParam();
  Q = new FakeParam();
  gain = new FakeParam();
  constructor() {
    super('filter');
  }
}

class FakeAnalyser extends FakeNode {
  fftSize = 2048;
  level = 0;
  constructor() {
    super('analyser');
  }
  getFloatTimeDomainData(buffer: Float32Array) {
    buffer.fill(this.level);
  }
}

const fakeContext = () => {
  const made: FakeNode[] = [];
  const keep = <T extends FakeNode>(node: T) => {
    made.push(node);
    return node;
  };
  const context = {
    sampleRate: 48_000,
    currentTime: 0,
    state: 'running',
    destination: new FakeNode('destination'),
    resume: () => Promise.resolve(),
    createGain: () => keep(new FakeGain()),
    createBiquadFilter: () => keep(new FakeFilter()),
    createConvolver: () => keep(Object.assign(new FakeNode('convolver'), { buffer: null })),
    createStereoPanner: () => keep(Object.assign(new FakeNode('panner'), { pan: new FakeParam() })),
    createDynamicsCompressor: () =>
      keep(
        Object.assign(new FakeNode('compressor'), {
          threshold: new FakeParam(),
          knee: new FakeParam(),
          ratio: new FakeParam(),
          attack: new FakeParam(),
          release: new FakeParam(),
        }),
      ),
    createMediaElementSource: () => keep(new FakeNode('source')),
    createChannelSplitter: () => keep(new FakeNode('splitter')),
    createAnalyser: () => keep(new FakeAnalyser()),
    createBuffer: (channels: number, length: number) => {
      const data = Array.from({ length: channels }, () => new Float32Array(length));
      return { getChannelData: (channel: number) => data[channel] };
    },
  };
  return { context, made };
};

const build = () => {
  const { context, made } = fakeContext();
  const sound = new WebAudioSound(context as unknown as AudioContext);
  const filters = made.filter((node): node is FakeFilter => node instanceof FakeFilter);
  const bands = filters.slice(0, EQ_FREQUENCIES.length);
  const gains = made.filter((node): node is FakeGain => node instanceof FakeGain);
  return { sound, made, bands, bass: filters[EQ_FREQUENCIES.length], gains, context };
};

describe('The Web Audio sound graph', () => {
  it('lays out ten bands: a low shelf, octave-wide peaks, a high shelf', () => {
    const { bands } = build();
    expect(bands.map((band) => band.type)).toEqual([
      'lowshelf',
      ...Array<string>(8).fill('peaking'),
      'highshelf',
    ]);
    expect(bands.map((band) => band.frequency.value)).toEqual(EQ_FREQUENCIES);
    // Q = 2f / (f₊ − f₋): 2·1000 / (2000 − 500).
    expect(bands[5]?.Q.value).toBeCloseTo(4 / 3, 5);
  });

  it('applies the equalizer with headroom, bass, reverb, balance and mono', () => {
    const { sound, bands, bass, gains, made } = build();
    const preamp = gains[1];
    const eqBands = [60, 30, 0, 0, -20, 0, 0, 0, 40, 0];

    sound.apply({ ...SOUND_DEFAULTS, eqEnabled: true, eqBands, preamp: 20 });
    expect(bands.map((band) => band.gain.value)).toEqual([6, 3, 0, 0, -2, 0, 0, 0, 4, 0]);
    // +2 dB preamp, less the largest boost (6 dB).
    expect(preamp?.gain.value).toBeCloseTo(dbToGain(2 - 6), 6);

    sound.apply({
      ...SOUND_DEFAULTS,
      bassBoost: 50,
      reverb: 'largeHall',
      balance: -40,
      mono: true,
    });
    expect(bands.every((band) => band.gain.value === 0)).toBe(true);
    expect(bass?.gain.value).toBe(6);
    expect(preamp?.gain.value).toBeCloseTo(dbToGain(-6), 6);
    const convolver = made.find((node) => node.kind === 'convolver') as unknown as {
      buffer: unknown;
    };
    expect(convolver.buffer).not.toBeNull();
    const panner = made.find((node) => node.kind === 'panner') as unknown as { pan: FakeParam };
    expect(panner.pan.value).toBe(-0.4);
    const mono = gains.find((node) => node.channelCountMode === 'explicit');
    expect(mono?.channelCount).toBe(1);

    sound.apply(SOUND_DEFAULTS);
    expect(convolver.buffer).toBeNull();
    expect(gains.every((node) => node.channelCountMode === 'max')).toBe(true);
  });

  it('works out headroom from the biggest boost', () => {
    expect(
      headroomDb({
        ...SOUND_DEFAULTS,
        eqEnabled: true,
        eqBands: [0, 0, 90, 0, 0, 0, 0, 0, 0, -50],
      }),
    ).toBe(9);
    expect(headroomDb({ ...SOUND_DEFAULTS, bassBoost: 100 })).toBe(12);
    expect(headroomDb(SOUND_DEFAULTS)).toBe(0);
  });

  it('gives each deck a level, an equal-power fade and a meter', () => {
    const { sound, made } = build();
    const deck = sound.deck({} as HTMLMediaElement);
    const [level, fade] = made.filter((node) => node instanceof FakeGain).slice(-2) as FakeGain[];

    deck.setGain(-6);
    expect(level?.gain.value).toBeCloseTo(dbToGain(-6), 6);
    deck.setFade(0);
    deck.fadeTo(1, 3000);
    const curve = fade?.gain.curve;
    expect(curve?.[0]).toBe(0);
    expect(curve?.[curve.length - 1]).toBeCloseTo(1, 6);
    // Equal power: halfway in time is at sin(45°), not 0.5.
    expect(curve?.[Math.round((curve.length - 1) / 2)]).toBeCloseTo(Math.SQRT1_2, 1);

    for (const analyser of made.filter((node) => node instanceof FakeAnalyser)) {
      (analyser as FakeAnalyser).level = 0.5;
    }
    // Both channels' power, summed.
    expect(deck.power()).toBeCloseTo(0.5, 6);
  });
});

describe('The loudness meter', () => {
  it('integrates gated 400 ms blocks', () => {
    const meter = new LoudnessMeter();
    expect(meter.integrated()).toBeNull();
    const power = 10 ** ((-14 + 0.691) / 10);
    for (let step = 0; step < 300; step += 1) meter.add(power);
    // Silence (below −70 LUFS) and a quiet stretch (more than 10 LU down) don't pull it down.
    for (let step = 0; step < 300; step += 1) meter.add(0);
    for (let step = 0; step < 100; step += 1) meter.add(power / 1000);
    expect(meter.integrated()).toBeCloseTo(-14, 1);
    expect(meter.seconds).toBeCloseTo(69.7, 1);
  });

  it('turns samples into power and power into LUFS', () => {
    expect(meanSquare([0.5, -0.5, 0.5, -0.5])).toBe(0.25);
    expect(toLufs(1)).toBeCloseTo(-0.691, 3);
  });
});
