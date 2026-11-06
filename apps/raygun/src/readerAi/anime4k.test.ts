import { evaluateSize, parsePasses } from './anime4k';

// Two passes as Anime4K writes them: one at the picture's size, one twice as big into MAIN.
const SHADER = `// MIT License
//!DESC Conv
//!HOOK MAIN
//!BIND MAIN
//!SAVE conv2d_tf
//!WIDTH MAIN.w
//!HEIGHT MAIN.h
//!COMPONENTS 4
vec4 hook() {
    return MAIN_texOff(vec2(0.0, 0.0));
}
//!DESC Depth-to-Space
//!HOOK MAIN
//!BIND MAIN
//!BIND conv2d_tf
//!SAVE MAIN
//!WIDTH conv2d_tf.w 2 *
//!HEIGHT conv2d_tf.h 2 *
vec4 hook() {
    return conv2d_tf_tex(conv2d_tf_pos) + MAIN_tex(MAIN_pos);
}
`;

describe('Anime4K shaders', () => {
  it('read as passes with what they bind, save and how big they are', () => {
    const passes = parsePasses(SHADER);
    expect(passes).toHaveLength(2);
    expect(passes[0]).toMatchObject({ binds: ['MAIN'], save: 'conv2d_tf', width: 'MAIN.w' });
    expect(passes[0]!.code).toContain('MAIN_texOff');
    expect(passes[0]!.code).not.toContain('//!');
    expect(passes[1]).toMatchObject({ binds: ['MAIN', 'conv2d_tf'], save: 'MAIN' });
  });

  it('work out sizes from mpv’s reverse Polish expressions', () => {
    const sizes = new Map([
      ['MAIN', { w: 800, h: 1200 }],
      ['conv2d_tf', { w: 800, h: 1200 }],
    ]);
    expect(evaluateSize('MAIN.w', sizes)).toBe(800);
    expect(evaluateSize('conv2d_tf.h 2 *', sizes)).toBe(2400);
    expect(evaluateSize('MAIN.w 1.5 /', sizes)).toBe(533);
    expect(() => evaluateSize('other.w', sizes)).toThrow('unknown texture');
  });
});
