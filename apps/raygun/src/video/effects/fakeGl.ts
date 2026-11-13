/**
 * A stand-in for WebGL 2 in tests (jsdom has none): it compiles nothing, names each program by
 * the pass its shader is, and records each draw with where it went and what it read.
 */

type Program = { name: string; uniforms: Map<string, unknown> };
type Texture = { id: number };
type Framebuffer = { id: number; texture: Texture | null };

export type FakeDraw = {
  pass: string;
  /** The texture drawn into; null for the canvas. */
  into: Texture | null;
  viewport: [number, number, number, number];
  /** The textures bound to each sampler by name. */
  textures: Record<string, Texture | undefined>;
  uniforms: Record<string, unknown>;
};

const passOf = (source: string) => {
  if (source.includes('uBlocks')) return 'mean';
  if (source.includes('uHistory')) return 'clean';
  if (source.includes('uColour')) return 'final';
  if (source.includes('aFramePosition')) return 'vertex';
  return 'blocks';
};

export const fakeGl = () => {
  let ids = 0;
  let program: Program | null = null;
  let framebuffer: Framebuffer | null = null;
  let viewport: [number, number, number, number] = [0, 0, 0, 0];
  let unit = 0;
  const units = new Map<number, Texture>();
  const sources = new Map<object, string>();
  const draws: FakeDraw[] = [];
  const compiled: string[] = [];
  const gl = {
    VERTEX_SHADER: 1,
    FRAGMENT_SHADER: 2,
    COMPILE_STATUS: 3,
    LINK_STATUS: 4,
    ARRAY_BUFFER: 5,
    STATIC_DRAW: 6,
    TEXTURE_2D: 7,
    RGBA: 8,
    UNSIGNED_BYTE: 9,
    FRAMEBUFFER: 10,
    COLOR_ATTACHMENT0: 11,
    TEXTURE0: 100,
    TEXTURE_MIN_FILTER: 12,
    TEXTURE_MAG_FILTER: 13,
    TEXTURE_WRAP_S: 14,
    TEXTURE_WRAP_T: 15,
    CLAMP_TO_EDGE: 16,
    LINEAR: 17,
    NEAREST: 18,
    FLOAT: 19,
    TRIANGLE_STRIP: 20,
    UNPACK_FLIP_Y_WEBGL: 21,
    createShader: () => ({}),
    shaderSource: (shader: object, source: string) => {
      sources.set(shader, source);
      compiled.push(source);
    },
    compileShader: () => undefined,
    getShaderParameter: () => true,
    getShaderInfoLog: () => '',
    deleteShader: () => undefined,
    createProgram: (): Program => ({ name: '', uniforms: new Map() }),
    attachShader: (target: Program, shader: object) => {
      const pass = passOf(sources.get(shader) ?? '');
      if (pass !== 'vertex') target.name = pass;
    },
    linkProgram: () => undefined,
    getProgramParameter: () => true,
    getProgramInfoLog: () => '',
    deleteProgram: () => undefined,
    getAttribLocation: () => 0,
    getUniformLocation: (_target: Program, name: string) => ({ name }),
    useProgram: (target: Program) => {
      program = target;
    },
    createBuffer: () => ({ id: (ids += 1) }),
    bindBuffer: () => undefined,
    bufferData: () => undefined,
    deleteBuffer: () => undefined,
    enableVertexAttribArray: () => undefined,
    vertexAttribPointer: () => undefined,
    createTexture: (): Texture => ({ id: (ids += 1) }),
    bindTexture: (_target: number, texture: Texture) => units.set(unit, texture),
    activeTexture: (value: number) => {
      unit = value - 100;
    },
    texImage2D: () => undefined,
    texSubImage2D: () => undefined,
    texParameteri: () => undefined,
    pixelStorei: () => undefined,
    deleteTexture: () => undefined,
    createFramebuffer: (): Framebuffer => ({ id: (ids += 1), texture: null }),
    bindFramebuffer: (_target: number, target: Framebuffer | null) => {
      framebuffer = target;
    },
    framebufferTexture2D: (_a: number, _b: number, _c: number, texture: Texture) => {
      if (framebuffer) framebuffer.texture = texture;
    },
    deleteFramebuffer: () => undefined,
    viewport: (x: number, y: number, width: number, height: number) => {
      viewport = [x, y, width, height];
    },
    uniform1i: (location: { name: string }, value: number) =>
      program?.uniforms.set(location.name, { unit: value }),
    uniform1f: (location: { name: string }, value: number) =>
      program?.uniforms.set(location.name, value),
    uniform2f: (location: { name: string }, x: number, y: number) =>
      program?.uniforms.set(location.name, [x, y]),
    uniform3f: (location: { name: string }, x: number, y: number, z: number) =>
      program?.uniforms.set(location.name, [x, y, z]),
    uniformMatrix3fv: (location: { name: string }, _transpose: boolean, value: Float32Array) =>
      program?.uniforms.set(location.name, Array.from(value)),
    drawArrays: () => {
      const uniforms = Object.fromEntries(program?.uniforms ?? []);
      const textures: Record<string, Texture | undefined> = {};
      for (const [name, value] of Object.entries(uniforms)) {
        if (value && typeof value === 'object' && 'unit' in value) {
          textures[name] = units.get((value as { unit: number }).unit);
        }
      }
      draws.push({
        pass: program?.name ?? '',
        into: framebuffer?.texture ?? null,
        viewport,
        textures,
        uniforms,
      });
    },
    getExtension: () => null,
  };
  return { gl, draws, compiled };
};

/** A canvas whose `getContext('webgl2')` answers `gl` (null: no WebGL 2). */
export const canvasWith = (gl: unknown) => {
  const canvas = document.createElement('canvas');
  canvas.getContext = (() => gl) as unknown as HTMLCanvasElement['getContext'];
  return canvas;
};
