// zlib's DEFLATE decoder (RFC 1950 and 1951), for KuGou's compressed KRC lyrics: the extension
// runtime has no decompression of its own. Canonical Huffman codes are decoded a bit at a time,
// the way zlib's reference decoder `puff` does; lyrics files are a few kilobytes.

/** Reads a byte array's bits, least significant first. */
class Bits {
  private at = 0;
  private bit = 0;

  constructor(private readonly data: Uint8Array) {}

  read(count: number): number {
    let value = 0;
    for (let index = 0; index < count; index += 1) {
      if (this.at >= this.data.length) throw new Error('compressed data ends early');
      value |= (((this.data[this.at] ?? 0) >> this.bit) & 1) << index;
      this.bit += 1;
      if (this.bit === 8) {
        this.bit = 0;
        this.at += 1;
      }
    }
    return value;
  }

  /** Skips to the next whole byte and reads `count` bytes from there. */
  bytes(count: number): Uint8Array {
    if (this.bit > 0) {
      this.bit = 0;
      this.at += 1;
    }
    if (this.at + count > this.data.length) throw new Error('compressed data ends early');
    const slice = this.data.subarray(this.at, this.at + count);
    this.at += count;
    return slice;
  }
}

type Huffman = { counts: number[]; symbols: number[] };

const MAX_BITS = 15;

/** The canonical code for these code lengths (0 = symbol unused). */
const huffman = (lengths: number[]): Huffman => {
  const counts = new Array<number>(MAX_BITS + 1).fill(0);
  for (const length of lengths) counts[length] = (counts[length] ?? 0) + 1;
  counts[0] = 0;
  const offsets = new Array<number>(MAX_BITS + 2).fill(0);
  for (let length = 1; length <= MAX_BITS; length += 1) {
    offsets[length + 1] = (offsets[length] ?? 0) + (counts[length] ?? 0);
  }
  const symbols = new Array<number>(lengths.length).fill(0);
  lengths.forEach((length, symbol) => {
    if (length > 0) {
      symbols[offsets[length] ?? 0] = symbol;
      offsets[length] = (offsets[length] ?? 0) + 1;
    }
  });
  return { counts, symbols };
};

const decode = (bits: Bits, table: Huffman): number => {
  let code = 0;
  let first = 0;
  let index = 0;
  for (let length = 1; length <= MAX_BITS; length += 1) {
    code |= bits.read(1);
    const count = table.counts[length] ?? 0;
    if (code - first < count) return table.symbols[index + code - first] ?? 0;
    index += count;
    first = (first + count) << 1;
    code <<= 1;
  }
  throw new Error('compressed data is damaged');
};

const LENGTH_BASE = [
  3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
  163, 195, 227, 258,
];
const LENGTH_EXTRA = [
  0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE = [
  1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049,
  3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA = [
  0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13,
];
/** The order code length code lengths come in. */
const CODE_LENGTH_ORDER = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

const fixedTables = (): [Huffman, Huffman] => {
  const lengths: number[] = [];
  for (let symbol = 0; symbol < 288; symbol += 1) {
    lengths.push(symbol < 144 ? 8 : symbol < 256 ? 9 : symbol < 280 ? 7 : 8);
  }
  return [huffman(lengths), huffman(new Array<number>(30).fill(5))];
};

const dynamicTables = (bits: Bits): [Huffman, Huffman] => {
  const literals = bits.read(5) + 257;
  const distances = bits.read(5) + 1;
  const codes = bits.read(4) + 4;
  const codeLengths = new Array<number>(19).fill(0);
  for (let index = 0; index < codes; index += 1) {
    codeLengths[CODE_LENGTH_ORDER[index] ?? 0] = bits.read(3);
  }
  const lengthCode = huffman(codeLengths);
  const lengths: number[] = [];
  while (lengths.length < literals + distances) {
    const symbol = decode(bits, lengthCode);
    if (symbol < 16) {
      lengths.push(symbol);
      continue;
    }
    let repeat: number;
    let value = 0;
    if (symbol === 16) {
      if (lengths.length === 0) throw new Error('compressed data is damaged');
      value = lengths[lengths.length - 1] ?? 0;
      repeat = 3 + bits.read(2);
    } else if (symbol === 17) {
      repeat = 3 + bits.read(3);
    } else {
      repeat = 11 + bits.read(7);
    }
    for (let step = 0; step < repeat; step += 1) lengths.push(value);
  }
  if (lengths.length > literals + distances) throw new Error('compressed data is damaged');
  return [huffman(lengths.slice(0, literals)), huffman(lengths.slice(literals))];
};

/** Decompresses a zlib stream (a two-byte header, DEFLATE blocks, a checksum left unread). */
export const inflate = (data: Uint8Array): Uint8Array => {
  const method = data[0] ?? 0;
  const flags = data[1] ?? 0;
  if ((method & 0x0f) !== 8 || (method * 256 + flags) % 31 !== 0 || flags & 0x20) {
    throw new Error('not zlib data');
  }
  const bits = new Bits(data.subarray(2));
  let out = new Uint8Array(Math.max(1024, data.length * 4));
  let size = 0;
  const push = (byte: number) => {
    if (size === out.length) {
      const grown = new Uint8Array(out.length * 2);
      grown.set(out);
      out = grown;
    }
    out[size] = byte;
    size += 1;
  };
  let fixed: [Huffman, Huffman] | null = null;
  let last = 0;
  while (!last) {
    last = bits.read(1);
    const type = bits.read(2);
    if (type === 0) {
      const header = bits.bytes(4);
      const length = (header[0] ?? 0) | ((header[1] ?? 0) << 8);
      for (const byte of bits.bytes(length)) push(byte);
      continue;
    }
    if (type === 3) throw new Error('compressed data is damaged');
    const [literal, distance] = type === 1 ? (fixed ??= fixedTables()) : dynamicTables(bits);
    for (;;) {
      const symbol = decode(bits, literal);
      if (symbol < 256) {
        push(symbol);
        continue;
      }
      if (symbol === 256) break;
      const lengthIndex = symbol - 257;
      if (lengthIndex >= LENGTH_BASE.length) throw new Error('compressed data is damaged');
      const length = (LENGTH_BASE[lengthIndex] ?? 0) + bits.read(LENGTH_EXTRA[lengthIndex] ?? 0);
      const distanceIndex = decode(bits, distance);
      if (distanceIndex >= DISTANCE_BASE.length) throw new Error('compressed data is damaged');
      const back =
        (DISTANCE_BASE[distanceIndex] ?? 0) + bits.read(DISTANCE_EXTRA[distanceIndex] ?? 0);
      if (back > size) throw new Error('compressed data is damaged');
      for (let step = 0; step < length; step += 1) push(out[size - back] ?? 0);
    }
  }
  return out.subarray(0, size);
};
