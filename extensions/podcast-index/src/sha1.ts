// SHA-1 (FIPS 180-4) as lowercase hex, for the Podcast Index API's request signature. Extensions
// run in QuickJS, which has no crypto API, and the signature needs nothing faster.

const rotate = (value: number, by: number) => (value << by) | (value >>> (32 - by));

/** `text` as UTF-8 bytes. */
const utf8 = (text: string) => {
  const bytes: number[] = [];
  for (const char of text) {
    const code = char.codePointAt(0) ?? 0;
    if (code < 0x80) {
      bytes.push(code);
    } else if (code < 0x800) {
      bytes.push(0xc0 | (code >> 6), 0x80 | (code & 63));
    } else if (code < 0x10000) {
      bytes.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63));
    } else {
      bytes.push(
        0xf0 | (code >> 18),
        0x80 | ((code >> 12) & 63),
        0x80 | ((code >> 6) & 63),
        0x80 | (code & 63),
      );
    }
  }
  return bytes;
};

export const sha1Hex = (text: string): string => {
  const bytes = utf8(text);
  // The message, a 1 bit, zeros, and its length in bits, in 64-byte blocks of 32-bit words.
  const blocks = ((bytes.length + 8) >> 6) + 1;
  const words = new Array<number>(blocks * 16).fill(0);
  bytes.forEach((byte, index) => {
    words[index >> 2] = (words[index >> 2] ?? 0) | (byte << (24 - (index % 4) * 8));
  });
  words[bytes.length >> 2] =
    (words[bytes.length >> 2] ?? 0) | (0x80 << (24 - (bytes.length % 4) * 8));
  const bits = bytes.length * 8;
  words[blocks * 16 - 2] = Math.floor(bits / 0x100000000);
  words[blocks * 16 - 1] = bits >>> 0;

  const hash = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
  const schedule = new Array<number>(80).fill(0);
  for (let block = 0; block < words.length; block += 16) {
    for (let t = 0; t < 80; t += 1) {
      schedule[t] =
        t < 16
          ? (words[block + t] ?? 0)
          : rotate(
              (schedule[t - 3] ?? 0) ^
                (schedule[t - 8] ?? 0) ^
                (schedule[t - 14] ?? 0) ^
                (schedule[t - 16] ?? 0),
              1,
            );
    }
    let [a, b, c, d, e] = hash as [number, number, number, number, number];
    for (let t = 0; t < 80; t += 1) {
      const mix =
        t < 20
          ? (b & c) | (~b & d)
          : t < 40
            ? b ^ c ^ d
            : t < 60
              ? (b & c) | (b & d) | (c & d)
              : b ^ c ^ d;
      const constant = t < 20 ? 0x5a827999 : t < 40 ? 0x6ed9eba1 : t < 60 ? 0x8f1bbcdc : 0xca62c1d6;
      const next = (rotate(a, 5) + mix + e + constant + (schedule[t] ?? 0)) | 0;
      e = d;
      d = c;
      c = rotate(b, 30);
      b = a;
      a = next;
    }
    hash[0] = ((hash[0] ?? 0) + a) | 0;
    hash[1] = ((hash[1] ?? 0) + b) | 0;
    hash[2] = ((hash[2] ?? 0) + c) | 0;
    hash[3] = ((hash[3] ?? 0) + d) | 0;
    hash[4] = ((hash[4] ?? 0) + e) | 0;
  }
  return hash.map((word) => (word >>> 0).toString(16).padStart(8, '0')).join('');
};
