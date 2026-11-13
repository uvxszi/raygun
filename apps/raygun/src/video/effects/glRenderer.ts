import commonSource from './glsl/fx_common.glsl?raw';
import cleanSource from './glsl/fx_clean.glsl?raw';
import finalSource from './glsl/fx_final.glsl?raw';
import meanSource from './glsl/fx_stats_mean.glsl?raw';
import blocksSource from './glsl/fx_stats_blocks.glsl?raw';
import vertexSource from './glsl/fx_vertex.glsl?raw';
import { columns } from './effectParams';
import type { ShaderParams } from './effectParams';

/**
 * The picture's effects on desktop: the `<video>`'s frames drawn through the same GLSL passes as
 * Android's (`PlayerEffectsGl.kt`) with WebGL 2. Each new frame is copied into a texture, then the
 * deflicker's light (16 × 9 blocks, then 1 × 1), denoising into a history kept for the next frame,
 * and the final look onto the canvas.
 */

/** A pass's fragment shader: the shared part first, then its own. */
export const fragmentSource = (pass: string) => `${commonSource}\n${pass}`;

export const PASSES = {
  blocks: fragmentSource(blocksSource),
  mean: fragmentSource(meanSource),
  clean: fragmentSource(cleanSource),
  final: fragmentSource(finalSource),
} as const;

export const VERTEX_SOURCE = vertexSource;

type PassName = keyof typeof PASSES;

type Pass = {
  program: WebGLProgram;
  position: number;
  uniforms: Map<string, WebGLUniformLocation | null>;
};

type Target = {
  texture: WebGLTexture;
  framebuffer: WebGLFramebuffer;
  width: number;
  height: number;
};

const BLOCKS_X = 16;
const BLOCKS_Y = 9;

const compile = (gl: WebGL2RenderingContext, type: number, source: string) => {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('No shader');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Effect shader: ${log ?? 'failed'}`);
  }
  return shader;
};

const link = (gl: WebGL2RenderingContext, fragment: string): Pass => {
  const program = gl.createProgram();
  const vertexShader = compile(gl, gl.VERTEX_SHADER, VERTEX_SOURCE);
  const fragmentShader = compile(gl, gl.FRAGMENT_SHADER, fragment);
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  gl.deleteShader(vertexShader);
  gl.deleteShader(fragmentShader);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`Effect program: ${log ?? 'failed'}`);
  }
  return {
    program,
    position: gl.getAttribLocation(program, 'aFramePosition'),
    uniforms: new Map(),
  };
};

export class EffectsRenderer {
  private readonly gl: WebGL2RenderingContext;
  private readonly passes: Record<PassName, Pass>;
  private readonly quad: WebGLBuffer;
  /** The video's frame, as uploaded. */
  private readonly frame: WebGLTexture;
  private frameWidth = 0;
  private frameHeight = 0;
  private blockTarget: Target | null = null;
  private stats: [Target, Target] | null = null;
  private statsIndex = 0;
  private statsKnown = false;
  private cleaned: [Target, Target] | null = null;
  private cleanIndex = 0;
  private historyKnown = false;
  /** The last frame's media time: the same again is the same frame drawn again. */
  private lastTime = Number.NaN;

  constructor(private readonly canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: false,
    });
    if (!gl) throw new Error('No WebGL 2');
    this.gl = gl;
    this.passes = {
      blocks: link(gl, PASSES.blocks),
      mean: link(gl, PASSES.mean),
      clean: link(gl, PASSES.clean),
      final: link(gl, PASSES.final),
    };
    const quad = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, quad);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 0, 1, 1, -1, 0, 1, -1, 1, 0, 1, 1, 1, 0, 1]),
      gl.STATIC_DRAW,
    );
    this.quad = quad;
    this.frame = gl.createTexture();
  }

  /**
   * Draws `video`'s current frame with `params`. `time` is the frame's media time: drawing the
   * same frame again (paused, a slider moved) starts from the same history, not from its own.
   */
  draw(video: HTMLVideoElement, params: ShaderParams, time: number) {
    const { gl } = this;
    const width = video.videoWidth;
    const height = video.videoHeight;
    if (!width || !height) return;
    if (width !== this.frameWidth || height !== this.frameHeight) this.resize(width, height);
    const again = time === this.lastTime;
    this.lastTime = time;

    // OpenGL's way up (origin at the bottom left), as Android's frames come to the passes.
    gl.bindTexture(gl.TEXTURE_2D, this.frame);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, video);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);

    let source = this.frame;
    let gainLeft = params.deflicker;
    if (params.deflicker) this.drawLight(params, again);
    else this.statsKnown = false;
    if (params.denoise) {
      source = this.drawClean(params, again);
      gainLeft = false;
    } else {
      this.historyKnown = false;
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, width, height);
    this.drawFinal(source, params, gainLeft);
  }

  /** A seek: the frames before it are no history for the ones after. */
  restart() {
    this.statsKnown = false;
    this.historyKnown = false;
    this.lastTime = Number.NaN;
  }

  dispose() {
    const { gl } = this;
    this.deleteFrames();
    for (const target of [this.blockTarget, ...(this.stats ?? [])]) {
      if (target) this.deleteTarget(target);
    }
    for (const pass of Object.values(this.passes)) gl.deleteProgram(pass.program);
    gl.deleteBuffer(this.quad);
    gl.deleteTexture(this.frame);
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  }

  private resize(width: number, height: number) {
    const { gl } = this;
    this.frameWidth = width;
    this.frameHeight = height;
    this.canvas.width = width;
    this.canvas.height = height;
    gl.bindTexture(gl.TEXTURE_2D, this.frame);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    this.deleteFrames();
    this.restart();
  }

  private target(width: number, height: number): Target {
    const { gl } = this;
    const texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    const framebuffer = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
    return { texture, framebuffer, width, height };
  }

  private deleteTarget(target: Target) {
    this.gl.deleteFramebuffer(target.framebuffer);
    this.gl.deleteTexture(target.texture);
  }

  private deleteFrames() {
    for (const target of this.cleaned ?? []) this.deleteTarget(target);
    this.cleaned = null;
    this.historyKnown = false;
  }

  private focus(target: Target) {
    this.gl.bindFramebuffer(this.gl.FRAMEBUFFER, target.framebuffer);
    this.gl.viewport(0, 0, target.width, target.height);
  }

  private use(pass: Pass) {
    const { gl } = this;
    gl.useProgram(pass.program);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.quad);
    gl.enableVertexAttribArray(pass.position);
    gl.vertexAttribPointer(pass.position, 4, gl.FLOAT, false, 0, 0);
  }

  private at(pass: Pass, name: string) {
    if (!pass.uniforms.has(name)) {
      pass.uniforms.set(name, this.gl.getUniformLocation(pass.program, name));
    }
    return pass.uniforms.get(name) ?? null;
  }

  private texture(pass: Pass, name: string, unit: number, texture: WebGLTexture, linear: boolean) {
    const { gl } = this;
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    const filter = linear ? gl.LINEAR : gl.NEAREST;
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.uniform1i(this.at(pass, name), unit);
  }

  private drawQuad() {
    this.gl.drawArrays(this.gl.TRIANGLE_STRIP, 0, 4);
  }

  /** The deflicker's light: this frame's mean luma and the steady light it's held to. */
  private drawLight(params: ShaderParams, again: boolean) {
    const { gl } = this;
    const grid = this.blockTarget ?? (this.blockTarget = this.target(BLOCKS_X, BLOCKS_Y));
    const stats = this.stats ?? (this.stats = [this.target(1, 1), this.target(1, 1)]);
    if (!again) this.statsIndex = 1 - this.statsIndex;
    const drawn = stats[this.statsIndex as 0 | 1];
    const before = stats[(1 - this.statsIndex) as 0 | 1];

    this.focus(grid);
    const { blocks, mean } = this.passes;
    this.use(blocks);
    this.texture(blocks, 'uFrame', 0, this.frame, true);
    this.drawQuad();

    this.focus(drawn);
    this.use(mean);
    this.texture(mean, 'uBlocks', 0, grid.texture, false);
    this.texture(mean, 'uBefore', 1, before.texture, false);
    gl.uniform1f(this.at(mean, 'uFollow'), params.follow);
    gl.uniform1f(this.at(mean, 'uFresh'), this.statsKnown ? 0 : 1);
    this.drawQuad();
    this.statsKnown = true;
  }

  private statsTexture() {
    return this.statsKnown && this.stats ? this.stats[this.statsIndex as 0 | 1].texture : null;
  }

  /** Denoising into this frame's target, which the final pass then reads. */
  private drawClean(params: ShaderParams, again: boolean) {
    const { gl } = this;
    if (!this.cleaned) {
      this.cleaned = [
        this.target(this.frameWidth, this.frameHeight),
        this.target(this.frameWidth, this.frameHeight),
      ];
      this.historyKnown = false;
    }
    if (!again) this.cleanIndex = 1 - this.cleanIndex;
    const drawn = this.cleaned[this.cleanIndex as 0 | 1];
    const history = this.cleaned[(1 - this.cleanIndex) as 0 | 1];
    const light = params.deflicker ? this.statsTexture() : null;
    const { clean } = this.passes;
    this.focus(drawn);
    this.use(clean);
    this.texture(clean, 'uFrame', 0, this.frame, true);
    this.texture(clean, 'uHistory', 1, history.texture, false);
    if (light) this.texture(clean, 'uStats', 2, light, false);
    gl.uniform2f(this.at(clean, 'uTexel'), 1 / this.frameWidth, 1 / this.frameHeight);
    gl.uniform1f(this.at(clean, 'uDeflicker'), light ? 1 : 0);
    gl.uniform1f(this.at(clean, 'uSpatial'), params.spatial);
    gl.uniform1f(this.at(clean, 'uSpatialSigma'), params.spatialSigma);
    gl.uniform1f(this.at(clean, 'uTemporal'), this.historyKnown ? params.temporal : 0);
    gl.uniform1f(this.at(clean, 'uTemporalSigma'), params.temporalSigma);
    this.drawQuad();
    this.historyKnown = true;
    return drawn.texture;
  }

  private drawFinal(source: WebGLTexture, params: ShaderParams, gain: boolean) {
    const { gl } = this;
    const { final } = this.passes;
    this.use(final);
    this.texture(final, 'uFrame', 0, source, true);
    const light = gain ? this.statsTexture() : null;
    if (light) this.texture(final, 'uStats', 1, light, false);
    gl.uniform2f(this.at(final, 'uTexel'), 1 / this.frameWidth, 1 / this.frameHeight);
    gl.uniform1f(this.at(final, 'uDeflicker'), light ? 1 : 0);
    gl.uniform1f(this.at(final, 'uSharpen'), params.sharpen);
    gl.uniformMatrix3fv(this.at(final, 'uColour'), false, columns(params.colour));
    const [red = 0, green = 0, blue = 0] = params.colour.offset;
    gl.uniform3f(this.at(final, 'uOffset'), red, green, blue);
    gl.uniform1f(this.at(final, 'uGamma'), params.gamma);
    gl.uniform1f(this.at(final, 'uLimit'), 1);
    this.drawQuad();
  }
}
