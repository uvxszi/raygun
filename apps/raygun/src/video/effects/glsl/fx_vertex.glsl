// The video player's effect passes (GLSL ES 1.00), shared by Android (Media3, PlayerEffectsGl.kt,
// read from the plugin's assets) and the desktop player (WebGL 2, glRenderer.ts): one quad over
// the frame, its texture coordinates following OpenGL's (origin at the bottom left).
attribute vec4 aFramePosition;
varying vec2 vTexCoord;

void main() {
  gl_Position = aFramePosition;
  vTexCoord = aFramePosition.xy * 0.5 + 0.5;
}
