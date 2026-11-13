import commonSource from './glsl/fx_common.glsl?raw';
import cleanSource from './glsl/fx_clean.glsl?raw';
import finalSource from './glsl/fx_final.glsl?raw';
import meanSource from './glsl/fx_stats_mean.glsl?raw';
import blocksSource from './glsl/fx_stats_blocks.glsl?raw';
import vertexSource from './glsl/fx_vertex.glsl?raw';
import motionCommonSource from './glsl/fx_motion_common.glsl?raw';
import motionCopySource from './glsl/fx_motion_copy.glsl?raw';
import motionErrorSource from './glsl/fx_motion_error.glsl?raw';
import motionLumaSource from './glsl/fx_motion_luma.glsl?raw';
import motionSearchSource from './glsl/fx_motion_search.glsl?raw';
import motionSmoothSource from './glsl/fx_motion_smooth.glsl?raw';
import motionWarpSource from './glsl/fx_motion_warp.glsl?raw';
import { columns } from './effectParams';
import type { ShaderParams } from './effectParams';
import { flowSize, LEVELS, RADII } from './motionMath';

/**
 * The picture's effects on desktop: the `<video>`'s frames drawn through the same GLSL passes as
 * Android's (`PlayerEffectsGl.kt`) with WebGL 2. Each new frame is copied into a texture, then the
 * deflicker's light (16 × 9 blocks, then 1 × 1), denoising into a history kept for the next frame,
 * and the final look onto the canvas. With smooth motion, the finished frames are kept instead and
 * the motion between the last two found (`fx_motion_*.glsl`, as Android's `PlayerEffectsMotion.kt`),
 * and each screen refresh draws the picture between them (`present`).
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

/** A smooth motion pass's fragment shader: the shared parts first, then its own. */
export const motionSource = (pass: string) => `${commonSource}\n${motionCommonSource}\n${pass}`;

export const MOTION_PASSES = {
  copy: motionSource(motionCopySource),
  luma: motionSource(motionLumaSource),
  search: motionSource(motionSearchSource),
  smooth: motionSource(motionSmoothSource),
  error: motionSource(motionErrorSource),
  warp: motionSource(motionWarpSource),
} as const;

type PassName = keyof typeof PASSES;
type MotionPassName = keyof typeof MOTION_PASSES;

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

/** Smooth motion's targets: the kept frames, their luma pyramids (finest first) and the flow. */
type Motion = {
  kept: [Target, Target];
  pyramids: [Target[], Target[]];
  /** The search's result per level, finest first. */
  search: Target[];
  refined: Target;
  smoothed: Target;
  error: Target;
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

  // Smooth motion: its passes (made when first used), the two finished frames kept, their luma
  // pyramids, and the flow between them.
  private motionPasses: Record<MotionPassName, Pass> | null = null;
  private motion: Motion | null = null;
  private latest = 0;
  private keptCount = 0;

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
    const again = this.upload(video, time);
    if (again === null) return;
    this.keptCount = 0;
    this.drawLook(params, again, null);
  }

  /**
   * Smooth motion: draws `video`'s frame with `params` into the kept frames (the same frame again
   * replaces the latest) and finds the motion from the one before. Nothing shows until `present`.
   */
  keep(video: HTMLVideoElement, params: ShaderParams, time: number) {
    const again = this.upload(video, time);
    if (again === null) return;
    const motion = this.motion ?? (this.motion = this.makeMotion());
    const replace = again && this.keptCount > 0;
    const slot = replace || this.keptCount === 0 ? this.latest : 1 - this.latest;
    const previous = 1 - slot;
    this.drawLook(params, again, motion.kept[slot as 0 | 1]);
    this.latest = slot;
    if (!replace) this.keptCount = Math.min(this.keptCount + 1, 2);
    this.drawPyramid(motion, slot);
    if (this.keptCount === 2) this.drawFlow(motion, previous, slot);
  }

  /** Two frames kept: `present` can draw between them. */
  get hasPair() {
    return this.keptCount === 2;
  }

  /**
   * Draws onto the canvas the picture `t` of the way from the frame before to the latest (the
   * latest alone without a pair).
   */
  present(t: number) {
    const { gl, motion } = this;
    if (!motion || this.keptCount === 0) return;
    const passes = this.motionPassesOf();
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, this.frameWidth, this.frameHeight);
    const latest = motion.kept[this.latest as 0 | 1];
    const before = motion.kept[(1 - this.latest) as 0 | 1];
    if (this.keptCount < 2 || t >= 0.995 || t <= 0.005) {
      const shown = this.keptCount < 2 || t >= 0.5 ? latest : before;
      this.use(passes.copy);
      this.texture(passes.copy, 'uCopyFrom', 0, shown.texture, false);
      this.drawQuad();
      return;
    }
    const { warp } = passes;
    this.use(warp);
    this.texture(warp, 'uPrev', 0, before.texture, true);
    this.texture(warp, 'uCur', 1, latest.texture, true);
    this.texture(warp, 'uFlow', 2, motion.smoothed.texture, true);
    this.texture(warp, 'uError', 3, motion.error.texture, false);
    gl.uniform1f(this.at(warp, 'uT'), t);
    gl.uniform2f(
      this.at(warp, 'uFlowTexel'),
      1 / motion.smoothed.width,
      1 / motion.smoothed.height,
    );
    this.drawQuad();
  }

  /** The video's frame into the texture; whether it's the same frame again (null: no frame yet). */
  private upload(video: HTMLVideoElement, time: number) {
    const { gl } = this;
    const width = video.videoWidth;
    const height = video.videoHeight;
    if (!width || !height) return null;
    if (width !== this.frameWidth || height !== this.frameHeight) this.resize(width, height);
    const again = time === this.lastTime;
    this.lastTime = time;
    // OpenGL's way up (origin at the bottom left), as Android's frames come to the passes.
    gl.bindTexture(gl.TEXTURE_2D, this.frame);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, video);
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);
    return again;
  }

  /** The picture's passes, the final look onto the canvas or into `into`. */
  private drawLook(params: ShaderParams, again: boolean, into: Target | null) {
    const { gl } = this;
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
    if (into) {
      this.focus(into);
    } else {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, this.frameWidth, this.frameHeight);
    }
    this.drawFinal(source, params, gainLeft);
  }

  /** A seek: the frames before it are no history for the ones after. */
  restart() {
    this.statsKnown = false;
    this.historyKnown = false;
    this.lastTime = Number.NaN;
    this.keptCount = 0;
  }

  private motionPassesOf() {
    if (!this.motionPasses) {
      const { gl } = this;
      this.motionPasses = {
        copy: link(gl, MOTION_PASSES.copy),
        luma: link(gl, MOTION_PASSES.luma),
        search: link(gl, MOTION_PASSES.search),
        smooth: link(gl, MOTION_PASSES.smooth),
        error: link(gl, MOTION_PASSES.error),
        warp: link(gl, MOTION_PASSES.warp),
      };
    }
    return this.motionPasses;
  }

  private makeMotion(): Motion {
    this.motionPassesOf();
    const { width, height } = flowSize(this.frameWidth, this.frameHeight);
    const level = (index: number) => ({
      width: Math.max(4, width >> index),
      height: Math.max(4, height >> index),
    });
    const levels = Array.from({ length: LEVELS }, (_, index) => level(index));
    const pyramid = () => levels.map((size) => this.target(size.width, size.height));
    return {
      kept: [
        this.target(this.frameWidth, this.frameHeight),
        this.target(this.frameWidth, this.frameHeight),
      ],
      pyramids: [pyramid(), pyramid()],
      search: pyramid(),
      refined: this.target(width, height),
      smoothed: this.target(width, height),
      error: this.target(1, 1),
    };
  }

  private deleteMotion() {
    const { motion } = this;
    if (!motion) return;
    for (const target of [
      ...motion.kept,
      ...motion.pyramids.flat(),
      ...motion.search,
      motion.refined,
      motion.smoothed,
      motion.error,
    ]) {
      this.deleteTarget(target);
    }
    this.motion = null;
    this.keptCount = 0;
  }

  private drawPyramid(motion: Motion, slot: number) {
    const { gl } = this;
    const { luma } = this.motionPassesOf();
    let from = motion.kept[slot as 0 | 1];
    const pyramid = motion.pyramids[slot as 0 | 1];
    pyramid.forEach((into, level) => {
      this.focus(into);
      this.use(luma);
      this.texture(luma, 'uLumaFrom', 0, from.texture, true);
      gl.uniform2f(this.at(luma, 'uSpread'), 0.25 / into.width, 0.25 / into.height);
      gl.uniform1f(this.at(luma, 'uPicture'), level === 0 ? 1 : 0);
      this.drawQuad();
      from = into;
    });
  }

  private drawFlow(motion: Motion, previous: number, latest: number) {
    const { gl } = this;
    const passes = this.motionPassesOf();
    let init: Target | null = null;
    for (let level = LEVELS - 1; level >= 0; level -= 1) {
      const into = motion.search[level] as Target;
      this.searchStep(
        motion,
        into,
        previous,
        latest,
        level,
        init,
        RADII[LEVELS - 1 - level] ?? 1,
        1,
      );
      init = into;
    }
    this.searchStep(motion, motion.refined, previous, latest, 0, init, 1, 0.5);

    const { smooth, error } = passes;
    this.focus(motion.smoothed);
    this.use(smooth);
    this.texture(smooth, 'uFlowIn', 0, motion.refined.texture, false);
    gl.uniform2f(this.at(smooth, 'uTexel'), 1 / motion.smoothed.width, 1 / motion.smoothed.height);
    this.drawQuad();

    this.focus(motion.error);
    this.use(error);
    this.texture(error, 'uCosts', 0, motion.smoothed.texture, false);
    this.drawQuad();
  }

  private searchStep(
    motion: Motion,
    into: Target,
    previous: number,
    latest: number,
    level: number,
    init: Target | null,
    radius: number,
    step: number,
  ) {
    const { gl } = this;
    const { search } = this.motionPassesOf();
    const before = motion.pyramids[previous as 0 | 1][level] as Target;
    const after = motion.pyramids[latest as 0 | 1][level] as Target;
    this.focus(into);
    this.use(search);
    this.texture(search, 'uPrev', 0, before.texture, true);
    this.texture(search, 'uCur', 1, after.texture, true);
    this.texture(search, 'uInit', 2, (init ?? after).texture, true);
    gl.uniform1f(this.at(search, 'uHasInit'), init ? 1 : 0);
    gl.uniform2f(this.at(search, 'uTexel'), 1 / into.width, 1 / into.height);
    gl.uniform1f(this.at(search, 'uScale'), 2 ** level);
    gl.uniform1f(this.at(search, 'uStep'), step);
    gl.uniform1f(this.at(search, 'uRadius'), radius);
    this.drawQuad();
  }

  dispose() {
    const { gl } = this;
    this.deleteFrames();
    this.deleteMotion();
    for (const pass of Object.values(this.motionPasses ?? {})) gl.deleteProgram(pass.program);
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
    this.deleteMotion();
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
