import { interleave, toBase64 } from './songCapture';

describe('Background capture (M46.49)', () => {
  it('sends a piece as 16-bit interleaved samples, cut to what played', () => {
    const left = new Float32Array([0, 0.5, -0.5, 1]);
    const right = new Float32Array([0, -0.25, 0.25, -1]);
    const more = new Float32Array([2, -2]);
    const blocks = [
      [left, right],
      [more, more],
    ];
    expect(Array.from(interleave(blocks, 0, 6))).toEqual([
      0, 0, 16_384, -8_192, -16_383, 8_192, 32_767, -32_767, 32_767, 32_767, -32_767, -32_767,
    ]);
    // Only frames 1 to 4, across both blocks.
    expect(Array.from(interleave(blocks, 1, 5))).toEqual([
      16_384, -8_192, -16_383, 8_192, 32_767, -32_767, 32_767, 32_767,
    ]);
    expect(interleave(blocks, 3, 3)).toHaveLength(0);
  });

  it('encodes samples little-endian in base64', () => {
    const samples = new Int16Array([1, -2, 0x1234]);
    const bytes = Uint8Array.from(atob(toBase64(samples)), (char) => char.charCodeAt(0));
    expect(Array.from(bytes)).toEqual([0x01, 0x00, 0xfe, 0xff, 0x34, 0x12]);
  });
});
