import { CipherState, Handshake, generateKeyPair } from './noise';

const bytes = (plain: string) => new TextEncoder().encode(plain);
const text = (sealed: Uint8Array) => new TextDecoder().decode(sealed);

describe('Noise XX', () => {
  it('lets two sides learn each other’s static keys and talk', () => {
    const alice = generateKeyPair();
    const bob = generateKeyPair();
    const initiator = new Handshake(true, alice);
    const responder = new Handshake(false, bob);

    const first = initiator.write();
    expect(first).toHaveLength(32);
    responder.read(first);
    const second = responder.write();
    // e, s (encrypted with its tag), and an empty payload's tag.
    expect(second).toHaveLength(32 + 48 + 16);
    initiator.read(second);
    const third = initiator.write(bytes('hi'));
    expect(new TextDecoder().decode(responder.read(third))).toBe('hi');

    const ours = initiator.transport();
    const theirs = responder.transport();
    expect([...ours.remoteStatic]).toEqual([...bob.publicKey]);
    expect([...theirs.remoteStatic]).toEqual([...alice.publicKey]);
    expect([...initiator.hash]).toEqual([...responder.hash]);

    for (const message of ['one', 'two', 'three']) {
      const sealed = ours.send.encrypt(new Uint8Array(0), bytes(message));
      expect(new TextDecoder().decode(theirs.receive.decrypt(new Uint8Array(0), sealed))).toBe(
        message,
      );
    }
    const back = theirs.send.encrypt(new Uint8Array(0), bytes('back'));
    expect(new TextDecoder().decode(ours.receive.decrypt(new Uint8Array(0), back))).toBe('back');
  });

  it('fails a handshake with a tampered message', () => {
    const initiator = new Handshake(true, generateKeyPair());
    const responder = new Handshake(false, generateKeyPair());
    responder.read(initiator.write());
    const second = responder.write();
    second[40] = (second[40] ?? 0) ^ 1;
    expect(() => initiator.read(second)).toThrow();
  });

  it('refuses a frame replayed or out of order', () => {
    const key = new Uint8Array(32).fill(7);
    const sender = new CipherState(key);
    const receiver = new CipherState(key);
    const first = sender.encrypt(new Uint8Array(0), bytes('a'));
    const second = sender.encrypt(new Uint8Array(0), bytes('b'));
    expect(() => receiver.decrypt(new Uint8Array(0), second)).toThrow();
    // A failed frame doesn't move the counter (Noise's rule): the right one still opens.
    expect(text(receiver.decrypt(new Uint8Array(0), first))).toBe('a');
    expect(text(receiver.decrypt(new Uint8Array(0), second))).toBe('b');
    expect(() => receiver.decrypt(new Uint8Array(0), first)).toThrow();
  });

  it('takes turns only in the pattern’s order', () => {
    const initiator = new Handshake(true, generateKeyPair());
    expect(() => initiator.read(new Uint8Array(96))).toThrow();
    const responder = new Handshake(false, generateKeyPair());
    expect(() => responder.write()).toThrow();
  });
});
