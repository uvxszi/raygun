// Deflicker, step 2 (drawn 1 × 1): the frame's mean luma from its blocks, and the steady light it
// is held to, a moving mean of the frames before. A cut or a seek starts it again from this frame.
uniform sampler2D uBlocks;
uniform sampler2D uBefore;
// How much of this frame the steady light takes.
uniform float uFollow;
// 1 when there's no history: the first frame, or after a seek.
uniform float uFresh;

void main() {
  float sum = 0.0;
  for (int row = 0; row < 9; row++) {
    for (int column = 0; column < 16; column++) {
      vec2 at = (vec2(float(column), float(row)) + 0.5) / vec2(16.0, 9.0);
      sum += unpack16(texture2D(uBlocks, at).rg);
    }
  }
  float mean = sum / 144.0;
  float steady = unpack16(texture2D(uBefore, vec2(0.5)).ba);
  float change = abs(mean - steady) / max(steady, 0.04);
  if (uFresh > 0.5 || change > 0.35) {
    steady = mean;
  } else {
    steady = mix(steady, mean, uFollow);
  }
  gl_FragColor = vec4(pack16(mean), pack16(steady));
}
