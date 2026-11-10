import script from '../../../../crates/raygun-app/src/music_downloads/capture/youtube_tap.js?raw';

/**
 * Raygun's tap inside YouTube's frame (M46.52), run against a stand-in for the frame: its window,
 * the player's <video> and Web Audio. The script itself is Rust's (an initialization script of
 * every frame); this checks what it does with what the player and the processor give it.
 */

const HOST = 'http://localhost:4000';
const TOP = 'http://tauri.localhost';
const RATE = 96_000;
const BLOCK = 16_384;

type Listener = (event: { type: string }) => void;

class FakeVideo {
  paused = true;
  seeking = false;
  ended = false;
  readyState = 4;
  playbackRate = 1;
  currentTime = 0;
  duration = 200;
  muted = false;
  preservesPitch = true;
  private listeners = new Map<string, Listener[]>();
  addEventListener(type: string, listener: Listener) {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }
  removeEventListener(type: string, listener: Listener) {
    this.listeners.set(
      type,
      (this.listeners.get(type) ?? []).filter((known) => known !== listener),
    );
  }
  emit(type: string) {
    for (const listener of this.listeners.get(type) ?? []) listener({ type });
  }
  pause() {
    this.paused = true;
  }
  captureStream() {
    return captured;
  }
}

class FakeNode {
  connect = vi.fn();
  disconnect = vi.fn();
  onaudioprocess: ((event: unknown) => void) | null = null;
}

const captured = {
  getAudioTracks: () => [{}],
  addEventListener: vi.fn(),
  removeEventListener: vi.fn(),
};

class FakeContext {
  static made: FakeContext[] = [];
  sampleRate: number;
  currentTime = 0;
  destination = {};
  processor = new FakeNode();
  constructor(options?: { sampleRate?: number }) {
    this.sampleRate = options?.sampleRate ?? 48_000;
    FakeContext.made.push(this);
  }
  createScriptProcessor() {
    return this.processor;
  }
  createMediaElementSource() {
    return new FakeNode();
  }
  createMediaStreamSource() {
    return new FakeNode();
  }
  resume() {
    return Promise.resolve();
  }
  close() {
    return Promise.resolve();
  }
}

/** One frame of YouTube's player, with the tap in it. */
const frame = (origins: string[] = [HOST, TOP], origin = 'https://www.youtube.com') => {
  const video = new FakeVideo();
  const parent = {};
  const top = { postMessage: vi.fn() };
  let onMessage: ((event: unknown) => void) | null = null;
  const window = {
    top,
    parent,
    addEventListener: (type: string, listener: (event: unknown) => void) => {
      if (type === 'message') onMessage = listener;
    },
  } as Record<string, unknown>;
  const document = {
    querySelector: (selector: string) => (selector.startsWith('video') ? video : null),
    documentElement: {},
  };
  FakeContext.made = [];
  new Function(
    'window',
    'location',
    'document',
    'AudioContext',
    'MutationObserver',
    'MediaStream',
    script,
  )(
    window,
    { origin, ancestorOrigins: origins },
    document,
    FakeContext,
    class {
      observe() {}
      disconnect() {}
    },
    class {},
  );
  const ask = (data: object, from: unknown = parent, at = HOST) =>
    onMessage?.({ source: from, origin: at, data: { raygunTap: true, ...data } });
  const posted = () =>
    top.postMessage.mock.calls.map(([message]) => message as Record<string, unknown>);
  /** The processor's next block, its first frame at the context's `at` s: `fill` per frame. */
  const block = (at: number, fill: (frame: number) => number) => {
    const context = FakeContext.made[0]!;
    const channel = Float32Array.from({ length: BLOCK }, (_, index) => fill(index));
    context.processor.onaudioprocess?.({
      inputBuffer: {
        sampleRate: context.sampleRate,
        length: BLOCK,
        numberOfChannels: 2,
        getChannelData: () => channel,
      },
      outputBuffer: { numberOfChannels: 2, getChannelData: () => new Float32Array(BLOCK) },
      playbackTime: at + (2 * BLOCK - 128) / context.sampleRate,
    });
  };
  return { video, top, parent, ask, posted, block };
};

describe('the YouTube tap', () => {
  it('stays idle outside Raygun’s own players', () => {
    for (const [origins, origin] of [
      [[HOST, TOP], 'https://example.com'],
      [['https://evil.example', TOP], 'https://www.youtube.com'],
      [[HOST, 'https://evil.example'], 'https://www.youtube.com'],
      [[TOP], 'https://www.youtube.com'],
    ] as const) {
      const tap = frame([...origins], origin);
      tap.ask({ type: 'start', session: 's', speed: 2, mode: 'graph' });
      expect(tap.posted()).toEqual([]);
    }
    // Only its parent, the embed host page, may ask.
    const tap = frame();
    tap.ask({ type: 'start', session: 's', speed: 2, mode: 'graph' }, {}, HOST);
    tap.ask({ type: 'start', session: 's', speed: 2, mode: 'graph' }, tap.parent, TOP);
    expect(tap.posted()).toEqual([]);
  });

  it('hears the player through Web Audio at twice its pace, placing each piece in the video', () => {
    const tap = frame();
    tap.ask({ type: 'start', session: 's1', speed: 2, mode: 'graph' });
    expect(tap.posted().map((message) => message.type)).toEqual(['armed', 'attached']);
    expect(FakeContext.made[0]!.sampleRate).toBe(RATE);
    expect(tap.video.preservesPitch).toBe(false);
    expect(tap.top.postMessage.mock.calls[0]![1]).toBe(TOP);

    // Before it plays: nothing is sent.
    tap.block(0, () => 0.5);
    expect(tap.posted()).toHaveLength(2);

    // It plays from 1.5 s at 2×, the context at 1 s.
    const context = FakeContext.made[0]!;
    Object.assign(tap.video, { paused: false, currentTime: 1.5, playbackRate: 2 });
    context.currentTime = 1;
    tap.video.emit('playing');
    tap.block(1, (index) => (index % 2 ? 0.5 : -0.5));
    tap.block(1 + BLOCK / RATE, () => 0.25);
    const sounds = tap.posted().filter((message) => message.type === 'sound');
    expect(sounds).toHaveLength(2);
    expect(sounds[0]).toMatchObject({ session: 's1', startMs: 1_500, rate: RATE, speed: 2 });
    // The next block goes on where the first ended: twice its length in the video.
    expect(sounds[1]!.startMs).toBeCloseTo(1_500 + (BLOCK / RATE) * 1000 * 2, 6);
    const samples = new Int16Array(sounds[0]!.pcm as ArrayBuffer);
    expect(samples.length).toBe(BLOCK * 2);
    expect([...samples.slice(0, 4)]).toEqual([-16383, -16383, 16384, 16384]);

    // At another speed (an ad, YouTube's own controls), nothing is placed until it's right.
    tap.video.playbackRate = 1;
    tap.video.emit('ratechange');
    tap.block(1 + (2 * BLOCK) / RATE, () => 0.25);
    expect(tap.posted().filter((message) => message.type === 'sound')).toHaveLength(2);
    expect(tap.video.playbackRate).toBe(2);
  });

  it('says when the player waits, and ends once the last sound came', () => {
    const tap = frame();
    tap.ask({ type: 'start', session: 's1', speed: 2, mode: 'graph' });
    const context = FakeContext.made[0]!;
    Object.assign(tap.video, { paused: false, currentTime: 0, playbackRate: 2 });
    tap.video.emit('playing');
    tap.block(0, () => 0.1);
    tap.video.emit('waiting');
    expect(tap.posted().at(-1)!.type).toBe('stalled');

    Object.assign(tap.video, { currentTime: 200, ended: true, paused: true });
    context.currentTime = 0.2;
    tap.video.emit('ended');
    tap.block(BLOCK / RATE, () => 0.1);
    tap.block((2 * BLOCK) / RATE, () => 0.1);
    expect(tap.posted().at(-1)).toMatchObject({ type: 'ended', durationMs: 200_000 });
    // Stopped: its sound goes nowhere anymore.
    const count = tap.posted().length;
    tap.block((3 * BLOCK) / RATE, () => 0.1);
    expect(tap.posted()).toHaveLength(count);
  });

  it('mutes the player and reads its capture, and says so when that only gives silence', () => {
    const tap = frame();
    tap.ask({ type: 'start', session: 's2', speed: 1, mode: 'muted' });
    expect(tap.video.muted).toBe(true);
    expect(FakeContext.made[0]!.sampleRate).toBe(48_000);
    Object.assign(tap.video, { paused: false, currentTime: 0, playbackRate: 1 });
    tap.video.emit('playing');
    // Zeros are held back...
    for (let index = 0; index < 80; index += 1) tap.block((index * BLOCK) / 48_000, () => 0);
    const failed = tap.posted().filter((message) => message.type === 'failed');
    // ...and after 12 s of only zeros, it says it hears nothing.
    expect(tap.posted().filter((message) => message.type === 'sound')).toEqual([]);
    expect(failed).toEqual([expect.objectContaining({ session: 's2', error: 'silent' })]);
  });

  it('sends the silence it held back once sound comes', () => {
    const tap = frame();
    tap.ask({ type: 'start', session: 's3', speed: 1, mode: 'muted' });
    Object.assign(tap.video, { paused: false, currentTime: 0, playbackRate: 1 });
    tap.video.emit('playing');
    tap.block(0, () => 0);
    tap.block(BLOCK / 48_000, () => 0.3);
    const sounds = tap.posted().filter((message) => message.type === 'sound');
    expect(sounds.map((message) => message.startMs)).toEqual([0, (BLOCK / 48_000) * 1000]);
  });
});
