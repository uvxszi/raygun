// The back-end channel to a linked app (raygun-sync remote.rs): the same Noise XX handshake as
// sync, then frames whose plaintext starts with a kind byte. JSON messages come in one or more
// parts; body pieces carry the request they answer.
import { SyncError, readFrame, writeFrame } from '../sync/channel';
import type { ByteStream } from '../sync/channel';
import { Handshake } from '../sync/noise';
import type { CipherState, KeyPair } from '../sync/noise';

export const KIND_MORE = 0;
export const KIND_LAST = 1;
export const KIND_BODY = 2;
/** Plaintext per frame, kind byte included. */
export const FRAME_PLAIN = 60_000;
/** Largest message taken from the app. */
const MAX_MESSAGE = 64 * 1024 * 1024;

export type Incoming =
  | { kind: 'json'; value: Record<string, unknown> }
  | { kind: 'body'; id: number; bytes: Uint8Array };

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const EMPTY = new Uint8Array(0);

export class RemoteChannel {
  private constructor(
    private readonly stream: ByteStream,
    private readonly sending: CipherState,
    private readonly receiving: CipherState,
  ) {}

  /** Connects as the initiator; returns the channel and the app's static key. */
  static async open(stream: ByteStream, identity: KeyPair) {
    const handshake = new Handshake(true, identity);
    writeFrame(stream, handshake.write());
    handshake.read(await readFrame(stream));
    writeFrame(stream, handshake.write());
    const transport = handshake.transport();
    return {
      channel: new RemoteChannel(stream, transport.send, transport.receive),
      remote: transport.remoteStatic,
    };
  }

  /** Sends one JSON message, in as many frames as it needs. */
  send(message: unknown) {
    const bytes = encoder.encode(JSON.stringify(message));
    const size = FRAME_PLAIN - 1;
    const count = Math.max(1, Math.ceil(bytes.length / size));
    for (let index = 0; index < count; index += 1) {
      const part = bytes.subarray(index * size, (index + 1) * size);
      const plain = new Uint8Array(part.length + 1);
      plain[0] = index + 1 < count ? KIND_MORE : KIND_LAST;
      plain.set(part, 1);
      writeFrame(this.stream, this.sending.encrypt(EMPTY, plain));
    }
  }

  /** The next message or body piece. */
  async next(): Promise<Incoming> {
    const parts: Uint8Array[] = [];
    let total = 0;
    for (;;) {
      const plain = this.receiving.decrypt(EMPTY, await readFrame(this.stream));
      const kind = plain[0];
      if (kind === undefined) throw new SyncError('empty frame');
      if (kind === KIND_BODY) {
        if (parts.length > 0) throw new SyncError('a body piece inside a message');
        const id = new DataView(plain.buffer, plain.byteOffset + 1, 4).getUint32(0);
        return { kind: 'body', id, bytes: plain.slice(5) };
      }
      if (kind !== KIND_MORE && kind !== KIND_LAST) continue;
      const data = plain.subarray(1);
      parts.push(data);
      total += data.length;
      if (total > MAX_MESSAGE) throw new SyncError('message too large');
      if (kind === KIND_LAST) break;
    }
    const joined = new Uint8Array(total);
    let at = 0;
    for (const part of parts) {
      joined.set(part, at);
      at += part.length;
    }
    return { kind: 'json', value: JSON.parse(decoder.decode(joined)) as Record<string, unknown> };
  }
}
