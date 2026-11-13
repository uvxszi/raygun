// Smooth motion, step 1: a frame's luma at the flow's size, or a luma level at half its size, as
// the mean of four linear taps a quarter of a drawn pixel either way.
uniform sampler2D uLumaFrom;
// A quarter of a drawn pixel, in texture coordinates.
uniform vec2 uSpread;
// 1 when reading a picture (its luma is taken), 0 for a luma level (its r).
uniform float uPicture;

void main() {
  vec4 sum = texture2D(uLumaFrom, vTexCoord + vec2(-uSpread.x, -uSpread.y));
  sum += texture2D(uLumaFrom, vTexCoord + vec2(uSpread.x, -uSpread.y));
  sum += texture2D(uLumaFrom, vTexCoord + vec2(-uSpread.x, uSpread.y));
  sum += texture2D(uLumaFrom, vTexCoord + vec2(uSpread.x, uSpread.y));
  vec4 mean = sum * 0.25;
  float value = uPicture > 0.5 ? luma(mean.rgb) : mean.r;
  gl_FragColor = vec4(value, value, value, 1.0);
}
