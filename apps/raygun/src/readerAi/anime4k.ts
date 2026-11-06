/**
 * Runs Anime4K's shaders (MIT, github.com/bloc97/Anime4K) on a picture with WebGL 2: the readers'
 * upscale and restore layers. The shaders are mpv "user shaders": passes that each draw one
 * texture from others, named in `//!` lines (HOOK, BIND, SAVE, WIDTH, HEIGHT). Every pass here
 * draws into a half-float texture, as the networks' values go below zero; the last one is drawn
 * onto a canvas. One WebGL context serves every page, one picture at a time.
 */

type Pass = {
  binds: string[];
  save: string;
  width: string | null;
  height: string | null;
  code: string;
};

/** A shader file's passes, in order. */
export const parsePasses = (text: string): Pass[] => {
  const passes: Pass[] = [];
  let current: Pass | null = null;
  let inHeader = false;
  for (const line of text.split(/\r?\n/)) {
    const directive = line.match(/^\/\/!(\w+)\s*(.*)$/);
    if (directive) {
      if (!inHeader) {
        current = { binds: [], save: '', width: null, height: null, code: '' };
        passes.push(current);
        inHeader = true;
      }
      const [, name, value] = directive;
      const arg = (value ?? '').trim();
      if (name === 'BIND') current!.binds.push(arg);
      else if (name === 'SAVE') current!.save = arg;
      else if (name === 'WIDTH') current!.width = arg;
      else if (name === 'HEIGHT') current!.height = arg;
      continue;
    }
    if (current) {
      inHeader = false;
      current.code += `${line}\n`;
    }
  }
  for (const pass of passes) {
    if (!pass.save) pass.save = 'MAIN';
    if (!pass.binds.length) pass.binds.push('MAIN');
  }
  return passes;
};

type Size = { w: number; h: number };

/** mpv's size expressions: reverse Polish, over textures' `.w` and `.h` (`conv2d_tf.w 2 *`). */
export const evaluateSize = (expression: string, sizes: Map<string, Size>): number => {
  const stack: number[] = [];
  for (const token of expression.trim().split(/\s+/)) {
    const field = token.match(/^(\w+)\.(w|h|width|height)$/);
    if (field) {
      const size = sizes.get(field[1]!);
      if (!size) throw new Error(`unknown texture ${field[1]}`);
      stack.push(field[2]!.startsWith('w') ? size.w : size.h);
    } else if (/^-?\d+(\.\d+)?$/.test(token)) {
      stack.push(Number(token));
    } else {
      const right = stack.pop() ?? 0;
      const left = stack.pop() ?? 0;
      if (token === '+') stack.push(left + right);
      else if (token === '-') stack.push(left - right);
      else if (token === '*') stack.push(left * right);
      else if (token === '/') stack.push(left / right);
      else if (token === '>') stack.push(left > right ? 1 : 0);
      else if (token === '<') stack.push(left < right ? 1 : 0);
      else throw new Error(`unknown operator ${token}`);
    }
  }
  return Math.round(stack.pop() ?? 0);
};

const VERTEX = `#version 300 es
in vec2 corner;
out vec2 v_pos;
uniform float u_flip;
void main() {
  v_pos = corner * 0.5 + 0.5;
  if (u_flip > 0.5) v_pos.y = 1.0 - v_pos.y;
  gl_Position = vec4(corner, 0.0, 1.0);
}`;

/** A pass's fragment shader: mpv's names for each bound texture, then the pass's own code. */
const fragmentOf = (pass: Pass) => {
  const bound = [...new Set(pass.binds.map((name) => (name === 'HOOKED' ? 'MAIN' : name)))];
  const lines: string[] = [];
  for (const name of bound)
    lines.push(`uniform sampler2D ${name}_raw;`, `uniform vec2 ${name}_size;`);
  // HOOKED is the texture the pass hooks: MAIN, for every pass of these shaders.
  lines.push('#define HOOKED_raw MAIN_raw', '#define HOOKED_size MAIN_size');
  for (const name of [...bound, 'HOOKED']) {
    lines.push(
      `#define ${name}_pos v_pos`,
      `#define ${name}_pt (1.0 / ${name}_size)`,
      `#define ${name}_tex(p) texture(${name}_raw, (p))`,
      `#define ${name}_texOff(off) ${name}_tex(${name}_pos + ${name}_pt * vec2(off))`,
    );
  }
  return `#version 300 es
precision highp float;
precision highp sampler2D;
in vec2 v_pos;
out vec4 fragColor;
${lines.join('\n')}
${pass.code}
void main() { fragColor = hook(); }`;
};

const PRESENT = `#version 300 es
precision highp float;
in vec2 v_pos;
out vec4 fragColor;
uniform sampler2D MAIN_raw;
void main() { fragColor = vec4(clamp(texture(MAIN_raw, v_pos).rgb, 0.0, 1.0), 1.0); }`;

type Texture = { texture: WebGLTexture; w: number; h: number };

type Engine = {
  gl: WebGL2RenderingContext;
  canvas: HTMLCanvasElement;
  buffer: WebGLBuffer;
  framebuffer: WebGLFramebuffer;
  programs: Map<string, WebGLProgram>;
  maxSize: number;
};

let engine: Engine | null = null;
let failed: string | null = null;

const compile = (gl: WebGL2RenderingContext, vertex: string, fragment: string) => {
  const program = gl.createProgram()!;
  for (const [kind, source] of [
    [gl.VERTEX_SHADER, vertex],
    [gl.FRAGMENT_SHADER, fragment],
  ] as const) {
    const shader = gl.createShader(kind)!;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(shader);
      gl.deleteShader(shader);
      throw new Error(`The model's shader didn't compile: ${log}`);
    }
    gl.attachShader(program, shader);
  }
  gl.bindAttribLocation(program, 0, 'corner');
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    throw new Error(`The model's shader didn't link: ${gl.getProgramInfoLog(program)}`);
  }
  return program;
};

const start = (): Engine => {
  if (engine && !engine.gl.isContextLost()) return engine;
  if (failed) throw new Error(failed);
  const canvas = document.createElement('canvas');
  const gl = canvas.getContext('webgl2', {
    preserveDrawingBuffer: true,
    premultipliedAlpha: false,
    antialias: false,
  });
  const halfFloat =
    gl &&
    (gl.getExtension('EXT_color_buffer_float') ?? gl.getExtension('EXT_color_buffer_half_float'));
  if (!gl || !halfFloat) {
    failed = "This device's graphics can't run upscaling models";
    throw new Error(failed);
  }
  const buffer = gl.createBuffer()!;
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
  canvas.addEventListener('webglcontextlost', (event) => {
    event.preventDefault();
    engine = null;
  });
  engine = {
    gl,
    canvas,
    buffer,
    framebuffer: gl.createFramebuffer()!,
    programs: new Map(),
    maxSize: gl.getParameter(gl.MAX_TEXTURE_SIZE) as number,
  };
  return engine;
};

const newTexture = (gl: WebGL2RenderingContext, w: number, h: number, float: boolean) => {
  const texture = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  if (float) gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA16F, w, h, 0, gl.RGBA, gl.HALF_FLOAT, null);
  return texture;
};

const draw = (run: Engine, program: WebGLProgram, flip: boolean) => {
  const { gl } = run;
  gl.useProgram(program);
  gl.bindBuffer(gl.ARRAY_BUFFER, run.buffer);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  gl.uniform1f(gl.getUniformLocation(program, 'u_flip'), flip ? 1 : 0);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
};

/** Pictures larger than this many pixels aren't worked on (they're sharp enough, and memory). */
export const MAX_PIXELS = 8_000_000;

/**
 * The picture with a shader file's passes run on it: as big as the passes make it (twice for
 * the upscalers, the same for the restorers), on a canvas of its own.
 */
export const runShader = (id: string, shader: string, picture: ImageBitmap): HTMLCanvasElement => {
  const run = start();
  const { gl } = run;
  const passes = parsePasses(shader);
  const textures = new Map<string, Texture>();
  const made: WebGLTexture[] = [];
  try {
    const main = gl.createTexture()!;
    made.push(main);
    gl.bindTexture(gl.TEXTURE_2D, main);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, gl.RGBA, gl.UNSIGNED_BYTE, picture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    textures.set('MAIN', { texture: main, w: picture.width, h: picture.height });

    passes.forEach((pass, index) => {
      const sizes = new Map(
        [...textures].map(([name, value]) => [name, { w: value.w, h: value.h }]),
      );
      const first = textures.get(pass.binds[0] === 'HOOKED' ? 'MAIN' : pass.binds[0]!)!;
      const w = pass.width ? evaluateSize(pass.width, sizes) : first.w;
      const h = pass.height ? evaluateSize(pass.height, sizes) : first.h;
      if (w < 1 || h < 1 || w > run.maxSize || h > run.maxSize) {
        throw new Error('The picture is too large for this device to upscale');
      }
      const key = `${id}#${index}`;
      let program = run.programs.get(key);
      if (!program) {
        program = compile(gl, VERTEX, fragmentOf(pass));
        run.programs.set(key, program);
      }
      const target = newTexture(gl, w, h, true);
      made.push(target);
      gl.bindFramebuffer(gl.FRAMEBUFFER, run.framebuffer);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, target, 0);
      gl.viewport(0, 0, w, h);
      gl.useProgram(program);
      pass.binds.forEach((bound, unit) => {
        const name = bound === 'HOOKED' ? 'MAIN' : bound;
        const texture = textures.get(name);
        if (!texture) throw new Error(`The model reads ${name} before making it`);
        gl.activeTexture(gl.TEXTURE0 + unit);
        gl.bindTexture(gl.TEXTURE_2D, texture.texture);
        gl.uniform1i(gl.getUniformLocation(program, `${name}_raw`), unit);
        gl.uniform2f(gl.getUniformLocation(program, `${name}_size`), texture.w, texture.h);
      });
      draw(run, program, false);
      textures.set(pass.save, { texture: target, w, h });
    });

    const result = textures.get('MAIN')!;
    let present = run.programs.get('present');
    if (!present) {
      present = compile(gl, VERTEX, PRESENT);
      run.programs.set('present', present);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    run.canvas.width = result.w;
    run.canvas.height = result.h;
    gl.viewport(0, 0, result.w, result.h);
    gl.useProgram(present);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, result.texture);
    gl.uniform1i(gl.getUniformLocation(present, 'MAIN_raw'), 0);
    // The canvas shows its first row at the bottom; the picture's first row is its top.
    draw(run, present, true);
    // A copy, so the next picture can use the context.
    const out = document.createElement('canvas');
    out.width = result.w;
    out.height = result.h;
    out.getContext('2d')!.drawImage(run.canvas, 0, 0);
    return out;
  } finally {
    for (const texture of made) gl.deleteTexture(texture);
  }
};
