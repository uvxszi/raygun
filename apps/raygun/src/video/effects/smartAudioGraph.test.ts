import { act, renderHook, waitFor } from '@testing-library/react';

import { NO_SMART_AUDIO } from './effectParams';
import { dbToGain, VideoAudioGraph } from './smartAudio';
import { useVideoAudio } from './useVideoAudio';

/** An AudioParam that keeps the value last set or aimed at. */
class FakeParam {
  value = 0;
  cancelScheduledValues = () => this;
  setTargetAtTime = (value: number) => {
    this.value = value;
    return this;
  };
}

class FakeNode {
  channelCount = 2;
  channelCountMode = 'max';
  channelInterpretation = 'speakers';
  /** What it was connected to, until let go. */
  readonly targets: unknown[] = [];
  constructor(readonly kind: string) {}
  connect<T>(target: T) {
    this.targets.push(target);
    return target;
  }
  disconnect() {
    this.targets.length = 0;
  }
}

class FakeGain extends FakeNode {
  gain = new FakeParam();
  constructor() {
    super('gain');
    this.gain.value = 1;
  }
}

class FakeAnalyser extends FakeNode {
  fftSize = 2048;
  constructor(private readonly level: () => number) {
    super('analyser');
  }
  getFloatTimeDomainData(buffer: Float32Array) {
    buffer.fill(this.level());
  }
}

type FakeCompressor = FakeNode & {
  threshold: FakeParam;
  knee: FakeParam;
  ratio: FakeParam;
  attack: FakeParam;
  release: FakeParam;
};

/** Every context made, its nodes by kind, and a way to wake it. */
const contexts: FakeContext[] = [];
let sampleLevel = 0;
let startAwake = true;

class FakeContext {
  sampleRate = 48_000;
  currentTime = 0;
  state = startAwake ? 'running' : 'suspended';
  destination = new FakeNode('destination');
  readonly gains: FakeGain[] = [];
  readonly filters: (FakeNode & { gain: FakeParam; frequency: FakeParam; Q: FakeParam })[] = [];
  readonly compressors: FakeCompressor[] = [];
  readonly sourceNodes: FakeNode[] = [];
  sources = 0;
  constructor() {
    contexts.push(this);
  }
  resume = () => {
    this.state = 'running';
    return Promise.resolve();
  };
  close = () => Promise.resolve();
  createGain = () => {
    const gain = new FakeGain();
    this.gains.push(gain);
    return gain;
  };
  createBiquadFilter = () => {
    const filter = Object.assign(new FakeNode('filter'), {
      type: 'lowpass',
      gain: new FakeParam(),
      frequency: new FakeParam(),
      Q: new FakeParam(),
    });
    this.filters.push(filter);
    return filter;
  };
  createDynamicsCompressor = () => {
    const compressor: FakeCompressor = Object.assign(new FakeNode('compressor'), {
      threshold: new FakeParam(),
      knee: new FakeParam(),
      ratio: new FakeParam(),
      attack: new FakeParam(),
      release: new FakeParam(),
    });
    this.compressors.push(compressor);
    return compressor;
  };
  createMediaElementSource = () => {
    this.sources += 1;
    const source = new FakeNode('source');
    this.sourceNodes.push(source);
    return source;
  };
  createChannelSplitter = () => new FakeNode('splitter');
  createChannelMerger = () => new FakeNode('merger');
  createAnalyser = () => new FakeAnalyser(() => sampleLevel);
}

const media = () => {
  const element = document.createElement('video');
  Object.defineProperty(element, 'paused', { value: false });
  return element;
};

describe('smart audio on desktop', () => {
  beforeEach(() => {
    contexts.length = 0;
    sampleLevel = 0;
    startAwake = true;
    vi.stubGlobal('AudioContext', FakeContext);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('waits for the context to wake before taking the sound, keeping the volume meanwhile', async () => {
    startAwake = false;
    const video = media();
    const graph = new VideoAudioGraph(video);
    graph.setVolume(0.4);
    const [context] = contexts;
    // Asleep, the element keeps its own sound and volume.
    expect(context?.sources).toBe(0);
    expect(video.volume).toBeCloseTo(0.4);
    await vi.waitFor(() => expect(context?.sources).toBe(1));
    // Then the volume is the graph's last gain, after the loudness is measured.
    expect(video.volume).toBe(1);
    expect(context?.gains.at(-1)?.gain.value).toBeCloseTo(0.4);
    graph.setVolume(1.5);
    expect(context?.gains.at(-1)?.gain.value).toBeCloseTo(1.5);
  });

  it("takes the sound beside the picture through the same chain, at the graph's volume", () => {
    const graph = new VideoAudioGraph(media());
    graph.setVolume(0.5);
    const [context] = contexts;
    const input = context?.gains[0];
    const beside = document.createElement('audio');
    beside.volume = 0.5;
    expect(graph.attach(beside)).toBe(true);
    expect(context?.sources).toBe(2);
    expect(context?.sourceNodes[1]?.targets).toEqual([input]);
    expect(beside.volume).toBe(1);
    // Once per element.
    expect(graph.attach(beside)).toBe(true);
    expect(context?.sources).toBe(2);
    graph.detach(beside);
    expect(context?.sourceNodes[1]?.targets).toEqual([]);
  });

  it('says when the sound beside the picture goes through it, so its element plays at full', async () => {
    const video = media();
    const { result, rerender } = renderHook(
      ({ audio }) => useVideoAudio({ current: video }, 0.6, false, audio),
      { initialProps: { audio: NO_SMART_AUDIO } },
    );
    // No graph without smart audio or a boost: the element's own volume.
    expect(result.current.routed).toBe(false);
    expect(video.volume).toBeCloseTo(0.6);

    rerender({ audio: { ...NO_SMART_AUDIO, night: true } });
    await waitFor(() => expect(result.current.routed).toBe(true));
    expect(video.volume).toBe(1);
    const beside = document.createElement('audio');
    act(() => result.current.setExternal(beside));
    await waitFor(() => expect(contexts[0]?.sources).toBe(2));
    expect(result.current.routed).toBe(true);
    expect(beside.volume).toBe(1);
    act(() => result.current.setExternal(null));
    expect(contexts[0]?.sourceNodes[1]?.targets).toEqual([]);
  });

  it('sets its parts from the switches, passing the sound through when off', () => {
    const graph = new VideoAudioGraph(media());
    const [context] = contexts;
    const [presence] = context?.filters ?? [];
    const [compressor, limiter] = context?.compressors ?? [];
    expect(presence?.gain.value).toBe(0);
    expect(compressor?.ratio.value).toBe(1);
    expect(limiter?.threshold.value).toBe(-1);

    graph.setAudio({ ...NO_SMART_AUDIO, dialogue: true, night: true });
    expect(presence?.gain.value).toBe(5);
    expect(compressor?.ratio.value).toBe(3.5);
    expect(compressor?.threshold.value).toBe(-30);

    graph.setAudio(NO_SMART_AUDIO);
    expect(presence?.gain.value).toBe(0);
    expect(compressor?.ratio.value).toBe(1);
  });

  it('turns a quiet video up as it measures it', () => {
    vi.useFakeTimers();
    const graph = new VideoAudioGraph(media());
    // A constant level: −40 dBFS on each channel.
    sampleLevel = dbToGain(-40);
    graph.setAudio({ ...NO_SMART_AUDIO, normalize: true });
    vi.advanceTimersByTime(10_000);
    const level = contexts[0]?.gains.find((gain) => gain.gain.value > 1.5);
    expect(level?.gain.value).toBeGreaterThan(dbToGain(9));

    graph.setAudio(NO_SMART_AUDIO);
    expect(level?.gain.value).toBe(1);
    graph.close();
  });
});
