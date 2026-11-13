// Device sync's channel (raygun-sync channel.rs) over any byte pipe: the Noise XX handshake, then
// JSON messages in length-prefixed Noise frames. In the browser the pipe is a WebRTC data
// channel; the bytes on it are exactly what Raygun sends over TCP between apps.
import { parseJson, stringifyJson } from '../core/json';
import { Handshake } from './noise';
import type { CipherState, KeyPair } from './noise';
import type { Message } from './protocol';

/** Plaintext per frame, leaving room for the flag byte and the 16-byte tag. */
const CHUNK = 60_000;
/** Largest message accepted, against a peer that never ends one. */
const MAX_MESSAGE = 64 * 1024 * 1024;
/** A peer that goes quiet this long is gone. */
export const IO_TIMEOUT_MS = 30_000;

export class SyncError extends Error {}

/** Bytes that arrive in pieces and are read back in whatever sizes the reader asks for. */
export class ByteStream {
  private buffered: Uint8Array[] = [];
  private length = 0;
  private waiting: (() => void) | null = null;
  private closed: Error | null = null;

  constructor(
    /** Sends bytes to the other end. */
    readonly write: (bytes: Uint8Array) => void,
    private readonly timeoutMs = IO_TIMEOUT_MS,
  ) {}

  /** Bytes from the other end. */
  push(bytes: Uint8Array) {
    if (bytes.length === 0) return;
    this.buffered.push(bytes);
    this.length += bytes.length;
    this.wake();
  }

  /** The other end went away; readers waiting get `error`. */
  close(error = new SyncError('the other device closed the connection')) {
    this.closed ??= error;
    this.wake();
  }

  private wake() {
    const waiting = this.waiting;
    this.waiting = null;
    waiting?.();
  }

  private take(count: number): Uint8Array {
    const out = new Uint8Array(count);
    let at = 0;
    while (at < count) {
      const head = this.buffered[0] as Uint8Array;
      const used = Math.min(head.length, count - at);
      out.set(head.subarray(0, used), at);
      at += used;
      if (used === head.length) this.buffered.shift();
      else this.buffered[0] = head.subarray(used);
    }
    this.length -= count;
    return out;
  }

  async read(count: number): Promise<Uint8Array> {
    while (this.length < count) {
      if (this.closed) throw this.closed;
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(() => {
          this.waiting = null;
          reject(new SyncError("the other device didn't answer in time"));
        }, this.timeoutMs);
        this.waiting = () => {
          clearTimeout(timer);
          resolve();
        };
      });
    }
    return this.take(count);
  }
}

const writeFrame = (stream: ByteStream, data: Uint8Array) => {
  if (data.length > 0xffff) throw new SyncError('frame too large');
  const frame = new Uint8Array(2 + data.length);
  new DataView(frame.buffer).setUint16(0, data.length);
  frame.set(data, 2);
  stream.write(frame);
};

const readFrame = async (stream: ByteStream) => {
  const head = await stream.read(2);
  const length = new DataView(head.buffer, head.byteOffset).getUint16(0);
  return stream.read(length);
};

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export class Channel {
  private constructor(
    private readonly stream: ByteStream,
    private readonly sending: CipherState,
    private readonly receiving: CipherState,
  ) {}

  /** Connects as the initiator; returns the channel and the responder's static key. */
  static async initiate(stream: ByteStream, identity: KeyPair) {
    const handshake = new Handshake(true, identity);
    writeFrame(stream, handshake.write());
    handshake.read(await readFrame(stream));
    writeFrame(stream, handshake.write());
    const transport = handshake.transport();
    return {
      channel: new Channel(stream, transport.send, transport.receive),
      remote: transport.remoteStatic,
    };
  }

  /** Accepts as the responder; returns the channel and the initiator's static key. */
  static async respond(stream: ByteStream, identity: KeyPair) {
    const handshake = new Handshake(false, identity);
    handshake.read(await readFrame(stream));
    writeFrame(stream, handshake.write());
    handshake.read(await readFrame(stream));
    const transport = handshake.transport();
    return {
      channel: new Channel(stream, transport.send, transport.receive),
      remote: transport.remoteStatic,
    };
  }

  /** Sends one message, split into as many frames as it needs. */
  send(message: Message) {
    const bytes = encoder.encode(stringifyJson(message));
    const chunks: Uint8Array[] = [];
    for (let at = 0; at < bytes.length; at += CHUNK) chunks.push(bytes.subarray(at, at + CHUNK));
    if (chunks.length === 0) chunks.push(new Uint8Array(0));
    chunks.forEach((chunk, index) => {
      const plain = new Uint8Array(chunk.length + 1);
      plain[0] = index + 1 < chunks.length ? 1 : 0;
      plain.set(chunk, 1);
      writeFrame(this.stream, this.sending.encrypt(new Uint8Array(0), plain));
    });
  }

  async recv(): Promise<Message> {
    const parts: Uint8Array[] = [];
    let total = 0;
    for (;;) {
      const plain = this.receiving.decrypt(new Uint8Array(0), await readFrame(this.stream));
      if (plain.length === 0) throw new SyncError('empty frame');
      const data = plain.subarray(1);
      parts.push(data);
      total += data.length;
      if (total > MAX_MESSAGE) throw new SyncError('message too large');
      if (plain[0] === 0) break;
    }
    const joined = new Uint8Array(total);
    let at = 0;
    for (const part of parts) {
      joined.set(part, at);
      at += part.length;
    }
    return parseJson(decoder.decode(joined)) as Message;
  }
}
