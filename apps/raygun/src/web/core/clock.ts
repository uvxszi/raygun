// The hybrid logical clock and ids, as raygun-db makes them (ARCHITECTURE §10): a stamp is a u64
// whose high 48 bits are wall-clock milliseconds and low 16 bits a counter, so it's a BigInt here
// (it doesn't fit a JS number). Ids are UUIDv7.

const COUNTER_BITS = 16n;

export class Clock {
  constructor(
    private last: bigint,
    private readonly now: () => number = Date.now,
  ) {}

  get latest() {
    return this.last;
  }

  /** The stamp for a local write. */
  tick(): bigint {
    const wall = BigInt(Math.floor(this.now())) << COUNTER_BITS;
    this.last = wall > this.last + 1n ? wall : this.last + 1n;
    return this.last;
  }

  /** Folds in a stamp from another device, so later local writes order after it. */
  observe(remote: bigint): bigint {
    const wall = BigInt(Math.floor(this.now())) << COUNTER_BITS;
    let next = this.last + 1n;
    if (wall > next) next = wall;
    if (remote + 1n > next) next = remote + 1n;
    this.last = next;
    return this.last;
  }
}

const hex = (bytes: Uint8Array) =>
  Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');

/** A UUIDv7: 48 bits of milliseconds, then random bits, with the version and variant set. */
export const uuidV7 = (now: number = Date.now()): string => {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let time = BigInt(Math.floor(now));
  for (let index = 5; index >= 0; index -= 1) {
    bytes[index] = Number(time & 0xffn);
    time >>= 8n;
  }
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x70;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const text = hex(bytes);
  return `${text.slice(0, 8)}-${text.slice(8, 12)}-${text.slice(12, 16)}-${text.slice(16, 20)}-${text.slice(20)}`;
};
