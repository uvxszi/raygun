// The Noise handshake device sync speaks (raygun-sync channel.rs, through `snow`):
// Noise_XX_25519_ChaChaPoly_BLAKE2s with an empty prologue, per the Noise Protocol Framework
// (revision 34). After the handshake each side knows the other's static key, which is how paired
// devices recognise each other.
import { chacha20poly1305 } from '@noble/ciphers/chacha.js';
import { x25519 } from '@noble/curves/ed25519.js';
import { blake2s } from '@noble/hashes/blake2.js';
import { hmac } from '@noble/hashes/hmac.js';

export const PROTOCOL = 'Noise_XX_25519_ChaChaPoly_BLAKE2s';
const DHLEN = 32;
const TAG = 16;

/** A device's long-term X25519 key pair, which is also its identity. */
export type KeyPair = { privateKey: Uint8Array; publicKey: Uint8Array };

export const generateKeyPair = (): KeyPair => {
  const privateKey = x25519.utils.randomSecretKey();
  return { privateKey, publicKey: x25519.getPublicKey(privateKey) };
};

export const keyPairOf = (privateKey: Uint8Array): KeyPair => ({
  privateKey,
  publicKey: x25519.getPublicKey(privateKey),
});

const concat = (...parts: Uint8Array[]) => {
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
};

const hash = (data: Uint8Array) => blake2s(data);
const mac = (key: Uint8Array, data: Uint8Array) => hmac(blake2s, key, data);

/** HKDF with HMAC-BLAKE2s, two outputs (the only count XX needs). */
const hkdf = (chainingKey: Uint8Array, input: Uint8Array): [Uint8Array, Uint8Array] => {
  const temp = mac(chainingKey, input);
  const first = mac(temp, Uint8Array.of(1));
  const second = mac(temp, concat(first, Uint8Array.of(2)));
  return [first, second];
};

/** ChaChaPoly's nonce: 32 bits of zeros, then the counter as 64 bits little-endian. */
const nonceOf = (counter: bigint) => {
  const nonce = new Uint8Array(12);
  new DataView(nonce.buffer).setBigUint64(4, counter, true);
  return nonce;
};

/** One direction's key and counter. */
export class CipherState {
  private counter = 0n;
  constructor(private key: Uint8Array | null = null) {}

  hasKey() {
    return this.key !== null;
  }

  setKey(key: Uint8Array) {
    this.key = key;
    this.counter = 0n;
  }

  encrypt(ad: Uint8Array, plaintext: Uint8Array): Uint8Array {
    if (!this.key) return plaintext;
    const sealed = chacha20poly1305(this.key, nonceOf(this.counter), ad).encrypt(plaintext);
    this.counter += 1n;
    return sealed;
  }

  decrypt(ad: Uint8Array, ciphertext: Uint8Array): Uint8Array {
    if (!this.key) return ciphertext;
    const opened = chacha20poly1305(this.key, nonceOf(this.counter), ad).decrypt(ciphertext);
    this.counter += 1n;
    return opened;
  }
}

class SymmetricState {
  private chainingKey: Uint8Array;
  private handshakeHash: Uint8Array;
  readonly cipher = new CipherState();

  constructor() {
    const name = new TextEncoder().encode(PROTOCOL);
    // A name longer than the hash is hashed; a shorter one would be padded.
    this.handshakeHash =
      name.length > 32 ? hash(name) : concat(name, new Uint8Array(32 - name.length));
    this.chainingKey = this.handshakeHash;
  }

  mixHash(data: Uint8Array) {
    this.handshakeHash = hash(concat(this.handshakeHash, data));
  }

  mixKey(input: Uint8Array) {
    const [chainingKey, key] = hkdf(this.chainingKey, input);
    this.chainingKey = chainingKey;
    this.cipher.setKey(key);
  }

  encryptAndHash(plaintext: Uint8Array) {
    const ciphertext = this.cipher.encrypt(this.handshakeHash, plaintext);
    this.mixHash(ciphertext);
    return ciphertext;
  }

  decryptAndHash(ciphertext: Uint8Array) {
    const plaintext = this.cipher.decrypt(this.handshakeHash, ciphertext);
    this.mixHash(ciphertext);
    return plaintext;
  }

  split(): [CipherState, CipherState] {
    const [first, second] = hkdf(this.chainingKey, new Uint8Array(0));
    return [new CipherState(first), new CipherState(second)];
  }

  get hash() {
    return this.handshakeHash;
  }
}

/** The two directions once the handshake is done. */
export type Transport = { send: CipherState; receive: CipherState; remoteStatic: Uint8Array };

const dh = (keys: KeyPair, publicKey: Uint8Array) =>
  x25519.getSharedSecret(keys.privateKey, publicKey);

/**
 * One XX handshake: `-> e`, `<- e, ee, s, es`, `-> s, se`. Messages are written and read in
 * turn; after the third, `transport()` gives the two cipher states.
 */
export class Handshake {
  private readonly symmetric = new SymmetricState();
  private ephemeral: KeyPair | null = null;
  private remoteEphemeral: Uint8Array | null = null;
  private remoteStatic: Uint8Array | null = null;
  private step = 0;

  constructor(
    readonly initiator: boolean,
    private readonly identity: KeyPair,
    /** Fixed ephemeral keys, for test vectors only. */
    private readonly fixedEphemeral?: KeyPair,
  ) {
    // The prologue is empty, and is still mixed in.
    this.symmetric.mixHash(new Uint8Array(0));
  }

  private newEphemeral() {
    this.ephemeral = this.fixedEphemeral ?? generateKeyPair();
    return this.ephemeral;
  }

  private mine(): KeyPair {
    if (!this.ephemeral) throw new Error('no ephemeral key yet');
    return this.ephemeral;
  }

  private theirs(key: Uint8Array | null): Uint8Array {
    if (!key) throw new Error('the other device sent no key');
    return key;
  }

  /** Writes the next handshake message, carrying `payload`. */
  write(payload: Uint8Array = new Uint8Array(0)): Uint8Array {
    const symmetric = this.symmetric;
    let message: Uint8Array;
    if (this.initiator && this.step === 0) {
      const ephemeral = this.newEphemeral();
      symmetric.mixHash(ephemeral.publicKey);
      message = concat(ephemeral.publicKey, symmetric.encryptAndHash(payload));
    } else if (!this.initiator && this.step === 1) {
      const ephemeral = this.newEphemeral();
      symmetric.mixHash(ephemeral.publicKey);
      symmetric.mixKey(dh(ephemeral, this.theirs(this.remoteEphemeral)));
      const sealedStatic = symmetric.encryptAndHash(this.identity.publicKey);
      symmetric.mixKey(dh(this.identity, this.theirs(this.remoteEphemeral)));
      message = concat(ephemeral.publicKey, sealedStatic, symmetric.encryptAndHash(payload));
    } else if (this.initiator && this.step === 2) {
      const sealedStatic = symmetric.encryptAndHash(this.identity.publicKey);
      symmetric.mixKey(dh(this.identity, this.theirs(this.remoteEphemeral)));
      message = concat(sealedStatic, symmetric.encryptAndHash(payload));
    } else {
      throw new Error('not this side’s turn to write');
    }
    this.step += 1;
    return message;
  }

  /** Reads the other side's next handshake message; returns its payload. */
  read(message: Uint8Array): Uint8Array {
    const symmetric = this.symmetric;
    let payload: Uint8Array;
    if (!this.initiator && this.step === 0) {
      if (message.length < DHLEN) throw new Error('short handshake message');
      this.remoteEphemeral = message.slice(0, DHLEN);
      symmetric.mixHash(this.remoteEphemeral);
      payload = symmetric.decryptAndHash(message.slice(DHLEN));
    } else if (this.initiator && this.step === 1) {
      if (message.length < DHLEN * 2 + TAG) throw new Error('short handshake message');
      this.remoteEphemeral = message.slice(0, DHLEN);
      symmetric.mixHash(this.remoteEphemeral);
      symmetric.mixKey(dh(this.mine(), this.remoteEphemeral));
      this.remoteStatic = symmetric.decryptAndHash(message.slice(DHLEN, DHLEN * 2 + TAG));
      symmetric.mixKey(dh(this.mine(), this.remoteStatic));
      payload = symmetric.decryptAndHash(message.slice(DHLEN * 2 + TAG));
    } else if (!this.initiator && this.step === 2) {
      if (message.length < DHLEN + TAG) throw new Error('short handshake message');
      this.remoteStatic = symmetric.decryptAndHash(message.slice(0, DHLEN + TAG));
      symmetric.mixKey(dh(this.mine(), this.remoteStatic));
      payload = symmetric.decryptAndHash(message.slice(DHLEN + TAG));
    } else {
      throw new Error('not this side’s turn to read');
    }
    this.step += 1;
    return payload;
  }

  get finished() {
    return this.step === 3;
  }

  /** The handshake hash (channel binding; test vectors compare it). */
  get hash() {
    return this.symmetric.hash;
  }

  transport(): Transport {
    if (!this.finished) throw new Error('the handshake isn’t finished');
    const [first, second] = this.symmetric.split();
    return {
      send: this.initiator ? first : second,
      receive: this.initiator ? second : first,
      remoteStatic: this.theirs(this.remoteStatic),
    };
  }
}
