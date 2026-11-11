import { screen } from '@testing-library/react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { pcmSamples } from './useSpeechHost';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** jsdom has no speech: a stand-in that keeps what it was asked to say. */
class FakeUtterance {
  rate = 1;
  pitch = 1;
  volume = 1;
  lang = '';
  voice: unknown = null;
  onstart: (() => void) | null = null;
  onend: (() => void) | null = null;
  onboundary: ((event: { name: string; charIndex: number; charLength?: number }) => void) | null =
    null;
  onerror: ((event: { error: string }) => void) | null = null;
  constructor(public text: string) {}
}

const spoken: FakeUtterance[] = [];
const synth = {
  speak: vi.fn((utterance: FakeUtterance) => spoken.push(utterance)),
  cancel: vi.fn(),
  getVoices: vi.fn(() => [
    { lang: 'en-US', name: 'English' },
    { lang: 'de-DE', name: 'Deutsch' },
  ]),
};

/** jsdom has no Web Audio either: buffers and sources that keep what they were given. */
class FakeSource {
  constructor(public owner: FakeContext) {}
  buffer: { samples: Float32Array } | null = null;
  onended: (() => void) | null = null;
  started = false;
  stopped = false;
  connect = vi.fn();
  start = () => {
    this.started = true;
  };
  stop = () => {
    this.stopped = true;
  };
}

const sources: FakeSource[] = [];

class FakeGain {
  gain = { value: 1 };
  connect = vi.fn();
}

class FakeContext {
  destination = {};
  gains: FakeGain[] = [];
  suspend = vi.fn(async () => {});
  resume = vi.fn(async () => {});
  createGain = () => {
    const gain = new FakeGain();
    this.gains.push(gain);
    return gain;
  };
  createBuffer = (_channels: number, length: number, sampleRate: number) => {
    const samples = new Float32Array(length);
    return { sampleRate, samples, copyToChannel: (data: Float32Array) => samples.set(data) };
  };
  createBufferSource = () => {
    const source = new FakeSource(this);
    sources.push(source);
    return source;
  };
}

const reports = () =>
  commands.ttsReport.mock.calls.map(([json]) => JSON.parse(json as string) as unknown);

/** `samples` as the base64 16-bit PCM Rust sends. */
const pcm = (samples: number[]) =>
  btoa(
    String.fromCharCode(
      ...samples.flatMap((sample) => {
        const value = sample < 0 ? 0x10000 + sample : sample;
        return [value & 0xff, value >> 8];
      }),
    ),
  );

describe('Read-aloud on desktop', () => {
  beforeEach(() => {
    spoken.length = 0;
    sources.length = 0;
    Object.assign(window, {
      speechSynthesis: synth,
      SpeechSynthesisUtterance: FakeUtterance,
      AudioContext: FakeContext,
    });
    AppShellWrapper.seedCore();
  });

  afterEach(() => {
    delete (window as { speechSynthesis?: unknown }).speechSynthesis;
    delete (window as { SpeechSynthesisUtterance?: unknown }).SpeechSynthesisUtterance;
    delete (window as { AudioContext?: unknown }).AudioContext;
  });

  it('speaks the sentences Rust sends with a voice of the book’s language and reports words', async () => {
    await AppShellWrapper.mount('/');
    await screen.findByTestId('home-page');
    expect(commands.ttsSetAvailable).toHaveBeenCalledWith(true);

    emitEvent('speechCommandEvent', {
      type: 'speak',
      rate: 1.25,
      pitch: 0.9,
      volume: 0.6,
      language: 'de',
      items: [
        { id: '4:0', text: 'Kapitel eins.' },
        { id: '4:3:end', text: 'The last line.', language: 'en' },
      ],
    });
    expect(synth.cancel).toHaveBeenCalled();
    expect(spoken.map((utterance) => utterance.text)).toEqual(['Kapitel eins.', 'The last line.']);
    expect(spoken[0]).toMatchObject({ rate: 1.25, pitch: 0.9, volume: 0.6, lang: 'de-DE' });
    // A passage in another language is read by a voice of its own.
    expect(spoken[1]).toMatchObject({ volume: 0.6, lang: 'en-US' });

    spoken[0]!.onstart?.();
    spoken[0]!.onboundary?.({ name: 'word', charIndex: 8, charLength: 4 });
    spoken[0]!.onend?.();
    spoken[1]!.onstart?.();
    spoken[1]!.onend?.();
    expect(reports()).toEqual([
      { state: 'speaking', id: '4:0' },
      { state: 'range', id: '4:0', start: 8, end: 12 },
      { state: 'speaking', id: '4:3:end' },
      { state: 'ended', id: '4:3:end' },
    ]);

    // Stopping cancels; the cancelled pieces' callbacks no longer report.
    emitEvent('speechCommandEvent', { type: 'stop' });
    spoken[1]!.onerror?.({ error: 'interrupted' });
    spoken[0]!.onstart?.();
    expect(reports()).toHaveLength(4);
  });

  it('plays downloaded voices’ clips in order, pausing where they are', async () => {
    await AppShellWrapper.mount('/');
    await screen.findByTestId('home-page');

    emitEvent('speechCommandEvent', {
      type: 'clip',
      id: '7:0',
      sampleRate: 22050,
      pcm: pcm([0, 16384, -16384]),
      last: false,
    });
    emitEvent('speechCommandEvent', {
      type: 'clip',
      id: '7:1:end',
      sampleRate: 22050,
      pcm: pcm([1, 2]),
      last: true,
    });
    // The first plays at once; the second waits for it.
    expect(sources.map((source) => source.started)).toEqual([true]);
    expect(Array.from(sources[0]!.buffer!.samples)).toEqual([0, 0.5, -0.5]);
    expect(reports()).toEqual([{ state: 'speaking', id: '7:0' }]);

    // The player's own audio context (the music engine has its own) is held and let go.
    const context = sources[0]!.owner;
    // Its volume turns at once, the clip playing included.
    const [gain] = context.gains;
    expect(sources[0]!.connect).toHaveBeenCalledWith(gain);
    emitEvent('speechCommandEvent', { type: 'volume', volume: 0.4 });
    expect(gain!.gain.value).toBe(0.4);
    emitEvent('speechCommandEvent', { type: 'pause' });
    expect(context.suspend).toHaveBeenCalled();
    context.resume.mockClear();
    emitEvent('speechCommandEvent', { type: 'resume' });
    expect(context.resume).toHaveBeenCalled();

    sources[0]!.onended?.();
    expect(sources[1]!.started).toBe(true);
    sources[1]!.onended?.();
    expect(reports()).toEqual([
      { state: 'speaking', id: '7:0' },
      { state: 'speaking', id: '7:1:end' },
      { state: 'ended', id: '7:1:end' },
    ]);
  });

  it('reads base64 PCM as samples', () => {
    expect(Array.from(pcmSamples(pcm([32767, -32768, 0])))).toEqual([32767 / 32768, -1, 0]);
  });
});
