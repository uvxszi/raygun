// The part of Web Crypto plugins use (Spotify's web-player token, Subsonic's salts): random
// values and UUIDs, SHA digests, and HMAC keys that sign and verify. The work is done in Rust.
import { type Natives, unwrap } from './native';
import { base64ToBytes, bytesToBase64 } from './polyfills';

type Algorithm = string | { name: string; hash?: string | { name: string } };

type HmacKey = {
  type: 'secret';
  extractable: boolean;
  algorithm: { name: string; hash: { name: string } };
  usages: string[];
};

/** Raw key bytes, kept off the key object a plugin sees. */
const rawKeys = new WeakMap<HmacKey, Uint8Array>();

/** Web Crypto's limit for one `getRandomValues`. */
const MAX_RANDOM_BYTES = 65_536;

const nameOf = (algorithm: Algorithm | undefined, fallback: string) =>
  algorithm === undefined ? fallback : typeof algorithm === 'string' ? algorithm : algorithm.name;

const bytesOf = (data: unknown): Uint8Array => {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (ArrayBuffer.isView(data))
    return new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
  throw new TypeError('Expected an ArrayBuffer or a typed array');
};

const bufferOf = (bytes: Uint8Array) =>
  bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;

export const createCrypto = (natives: Natives) => {
  const run = (args: Record<string, unknown>) =>
    base64ToBytes(unwrap<string>(natives.crypto(JSON.stringify(args))));

  const random = (count: number) => base64ToBytes(unwrap<string>(natives.random(count)));

  const sign = (key: HmacKey, data: unknown) => {
    const raw = rawKeys.get(key);
    if (!raw) throw new TypeError('Not a key made by crypto.subtle.importKey');
    return run({
      op: 'hmac',
      algorithm: key.algorithm.hash.name,
      key: bytesToBase64(raw),
      data: bytesToBase64(bytesOf(data)),
    });
  };

  return {
    getRandomValues<T extends ArrayBufferView>(array: T): T {
      if (array.byteLength > MAX_RANDOM_BYTES) {
        throw new RangeError(`getRandomValues takes at most ${MAX_RANDOM_BYTES} bytes`);
      }
      new Uint8Array(array.buffer, array.byteOffset, array.byteLength).set(
        random(array.byteLength),
      );
      return array;
    },

    randomUUID(): string {
      const bytes = random(16);
      bytes[6] = ((bytes[6] as number) & 0x0f) | 0x40;
      bytes[8] = ((bytes[8] as number) & 0x3f) | 0x80;
      const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
      return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    },

    subtle: {
      async digest(algorithm: Algorithm, data: unknown): Promise<ArrayBuffer> {
        return bufferOf(
          run({
            op: 'digest',
            algorithm: nameOf(algorithm, 'SHA-256'),
            data: bytesToBase64(bytesOf(data)),
          }),
        );
      },

      async importKey(
        format: string,
        keyData: unknown,
        algorithm: Algorithm,
        extractable = false,
        usages: string[] = [],
      ): Promise<HmacKey> {
        const name = nameOf(algorithm, 'HMAC');
        if (format !== 'raw' || name.toUpperCase() !== 'HMAC') {
          throw new Error(`Raygun's crypto imports raw HMAC keys only, not ${format} ${name}`);
        }
        const hash = typeof algorithm === 'string' ? undefined : algorithm.hash;
        const key: HmacKey = {
          type: 'secret',
          extractable,
          algorithm: { name: 'HMAC', hash: { name: nameOf(hash, 'SHA-256') } },
          usages,
        };
        rawKeys.set(key, bytesOf(keyData).slice());
        return key;
      },

      async sign(_algorithm: Algorithm, key: HmacKey, data: unknown): Promise<ArrayBuffer> {
        return bufferOf(sign(key, data));
      },

      async verify(
        _algorithm: Algorithm,
        key: HmacKey,
        signature: unknown,
        data: unknown,
      ): Promise<boolean> {
        const expected = sign(key, data);
        const given = bytesOf(signature);
        return (
          expected.length === given.length && expected.every((byte, index) => byte === given[index])
        );
      },
    },
  };
};
