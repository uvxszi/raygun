// A part of the frame drawn into a small grid, for the watermark remover's estimate (Android reads
// it back; the desktop player samples the video with a 2D canvas instead).
uniform sampler2D uFrame;
// The part in texture coordinates: x0, y0, x1, y1 (origin at the bottom left).
uniform vec4 uRect;

void main() {
  gl_FragColor = vec4(texture2D(uFrame, mix(uRect.xy, uRect.zw, vTexCoord)).rgb, 1.0);
}
