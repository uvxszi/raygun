import { commands, events } from '@raygun/bindings';
import type { SongCaptureReport, SongCaptureRequest } from '@raygun/bindings';
import { useEffect } from 'react';

import { attachMedia, engineKind } from '../video/mediaEngine';
import type { MediaEngine } from '../video/mediaEngine';

/**
 * The desktop's background capture (M46.49): when no download path gives a song's file, Rust asks
 * for its stream (through the proxy's `/media/` route) to be captured. It plays muted in a media
 * element of its own, never the player people listen to, at 2× with its pitch left free, and is
 * recorded through Web Audio at twice a 48 kHz rate: read at half the context's rate, the samples
 * are the sound at its own pace, nothing stretched. Sound goes back in pieces that say where they
 * start in the stream; stalls, the end and failures are reported too.
 */

/** The sound's own rate the context runs at twice (or more) of. */
const SOUND_RATE = 48_000;
/** Context frames gathered before a piece is sent (about half a second at 96 kHz). */
const PIECE_FRAMES = 48_000;

const TAP = `
class RaygunCaptureTap extends AudioWorkletProcessor {
  constructor() {
    super();
    this.blocks = [];
    this.frames = 0;
    this.start = 0;
    this.port.onmessage = (event) => {
      if (event.data === 'flush') this.send(true);
    };
  }
  send(flushed) {
    this.port.postMessage({ blocks: this.blocks, frames: this.frames, start: this.start, flushed });
    this.blocks = [];
    this.frames = 0;
  }
  process(inputs) {
    const input = inputs[0];
    if (input && input.length > 0 && input[0].length > 0) {
      if (this.frames === 0) this.start = currentTime;
      this.blocks.push(input.map((channel) => channel.slice()));
      this.frames += input[0].length;
      if (this.frames >= ${PIECE_FRAMES}) this.send(false);
    }
    return true;
  }
}
registerProcessor('raygun-capture-tap', RaygunCaptureTap);
`;

type Piece = {
  blocks: Float32Array[][];
  frames: number;
  /** The context's time at its first frame. */
  start: number;
  flushed: boolean;
};

/** A stretch of steady playing: where the stream was at a context time, until it stopped. */
type Run = { streamMs: number; contextTime: number; until: number | null };

type Session = {
  id: string;
  element: HTMLVideoElement;
  context: AudioContext;
  speed: number;
  engine: MediaEngine | null;
  nodes: AudioNode[];
  runs: Run[];
  stopped: boolean;
  /** The reports on their way, one after another. */
  sending: Promise<void>;
  ending: boolean;
};

let current: Session | null = null;
let tapUrl: string | null = null;

/** Little-endian 16-bit samples as base64. */
export const toBase64 = (samples: Int16Array) => {
  const bytes = new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength);
  let text = '';
  for (let at = 0; at < bytes.length; at += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
  }
  return btoa(text);
};

/** Frames `from..to` of a piece's blocks, interleaved as 16-bit samples. */
export const interleave = (blocks: Float32Array[][], from: number, to: number) => {
  const channels = Math.max(1, blocks[0]?.length ?? 1);
  const out = new Int16Array(Math.max(0, to - from) * channels);
  let frame = 0;
  for (const block of blocks) {
    const length = block[0]?.length ?? 0;
    for (let index = 0; index < length; index += 1, frame += 1) {
      if (frame < from || frame >= to) continue;
      for (let channel = 0; channel < channels; channel += 1) {
        const value = Math.max(-1, Math.min(1, block[channel]?.[index] ?? 0));
        out[(frame - from) * channels + channel] = Math.round(value * 32_767);
      }
    }
  }
  return out;
};

/** Sends a report after the ones before it, so they arrive in order. */
const report = (session: Session, report: Omit<SongCaptureReport, 'session' | 'speed'>) => {
  session.sending = session.sending
    .then(() =>
      commands.songCaptureReport({ ...report, session: session.id, speed: session.speed }),
    )
    .then((result) => {
      // Rust stopped this capture.
      if (result.status === 'error') stop(session.id);
    })
    .catch(() => stop(session.id));
  return session.sending;
};

const durationOf = (session: Session) => {
  const seconds = session.element.duration;
  return Number.isFinite(seconds) && seconds > 0 ? seconds * 1000 : null;
};

/** Sends what of a piece played in a run, placed in the stream. */
const onPiece = (session: Session, piece: Piece) => {
  if (session.stopped) return;
  const rate = session.context.sampleRate;
  const end = piece.start + piece.frames / rate;
  for (const run of session.runs) {
    const from = Math.max(piece.start, run.contextTime);
    const to = Math.min(end, run.until ?? Infinity);
    if (to <= from) continue;
    const first = Math.round((from - piece.start) * rate);
    const last = Math.min(piece.frames, Math.round((to - piece.start) * rate));
    if (last <= first) continue;
    const startMs = run.streamMs + (from - run.contextTime) * 1000 * session.speed;
    void report(session, {
      kind: 'sound',
      startMs,
      rate,
      channels: Math.max(1, piece.blocks[0]?.length ?? 2),
      pcm: toBase64(interleave(piece.blocks, first, last)),
      durationMs: durationOf(session),
      error: null,
    });
  }
  // Runs that ended before this piece are done with.
  session.runs = session.runs.filter((run) => run.until === null || run.until > end);
  if (piece.flushed && session.ending) {
    session.ending = false;
    void report(session, {
      kind: 'ended',
      startMs: session.element.currentTime * 1000,
      rate,
      channels: 2,
      pcm: null,
      durationMs: durationOf(session),
      error: null,
    }).then(() => stop(session.id));
  }
};

const fail = (session: Session, error: string) => {
  if (session.stopped) return;
  void report(session, {
    kind: 'failed',
    startMs: session.element.currentTime * 1000,
    rate: session.context.sampleRate,
    channels: 2,
    pcm: null,
    durationMs: null,
    error,
  }).then(() => stop(session.id));
};

/** An audio context at twice the sound's rate, else one at the system's rate played at 1×. */
const openContext = (speed: number): [AudioContext, number] => {
  try {
    return [new AudioContext({ sampleRate: SOUND_RATE * speed }), speed];
  } catch {
    return [new AudioContext(), 1];
  }
};

export const start = async (request: SongCaptureRequest) => {
  if (!request.url) return;
  if (current) stop(current.id);
  const element = document.createElement('video');
  element.crossOrigin = 'anonymous';
  element.preload = 'auto';
  element.preservesPitch = false;
  const [context, speed] = openContext(Math.max(1, request.speed ?? 2));
  const session: Session = {
    id: request.session,
    element,
    context,
    speed,
    engine: null,
    nodes: [],
    runs: [],
    stopped: false,
    sending: Promise.resolve(),
    ending: false,
  };
  current = session;
  try {
    tapUrl ??= URL.createObjectURL(new Blob([TAP], { type: 'text/javascript' }));
    await context.audioWorklet.addModule(tapUrl);
    if (session.stopped) return;
    const source = context.createMediaElementSource(element);
    const tap = new AudioWorkletNode(context, 'raygun-capture-tap', {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [2],
      channelCount: 2,
      channelCountMode: 'explicit',
      channelInterpretation: 'speakers',
    });
    // Muted: nothing of it is heard.
    const silent = context.createGain();
    silent.gain.value = 0;
    source.connect(tap);
    tap.connect(silent);
    silent.connect(context.destination);
    session.nodes = [source, tap, silent];
    tap.port.onmessage = (event: MessageEvent<Piece>) => onPiece(session, event.data);
    const endRun = () => {
      const run = session.runs.at(-1);
      if (run && run.until === null) run.until = context.currentTime;
    };
    element.addEventListener('playing', () => {
      endRun();
      session.runs.push({
        streamMs: element.currentTime * 1000,
        contextTime: context.currentTime,
        until: null,
      });
    });
    element.addEventListener('waiting', () => {
      endRun();
      void report(session, {
        kind: 'stalled',
        startMs: element.currentTime * 1000,
        rate: context.sampleRate,
        channels: 2,
        pcm: null,
        durationMs: durationOf(session),
        error: null,
      });
    });
    element.addEventListener('ended', () => {
      endRun();
      session.ending = true;
      tap.port.postMessage('flush');
    });
    session.engine = await attachMedia(
      element,
      request.url,
      engineKind(request.url, request.format),
      {
        onTracks: () => {},
        onFatal: (message) => fail(session, message),
      },
    );
    if (session.stopped) return;
    element.defaultPlaybackRate = speed;
    element.playbackRate = speed;
    const fromMs = request.fromMs ?? 0;
    if (fromMs > 0) element.currentTime = fromMs / 1000;
    await context.resume();
    await element.play();
  } catch (error) {
    fail(session, error instanceof Error ? error.message : String(error));
  }
};

export const stop = (id: string) => {
  const session = current;
  if (!session || session.id !== id || session.stopped) return;
  session.stopped = true;
  current = null;
  session.element.pause();
  session.engine?.destroy();
  for (const node of session.nodes) node.disconnect();
  session.element.removeAttribute('src');
  session.element.load();
  void session.context.close().catch(() => undefined);
};

/** Captures what Rust asks for, on desktop (Android's capture player is native). */
export const useSongCaptureHost = (enabled: boolean) => {
  useEffect(() => {
    if (!enabled) return;
    const listening = events.songCaptureEvent.listen(({ payload }) => {
      if (payload.url) void start(payload);
      else stop(payload.session);
    });
    return () => {
      void listening.then((unlisten) => unlisten());
      if (current) stop(current.id);
    };
  }, [enabled]);
};
