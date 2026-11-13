// Deflicker, step 1 (drawn 16 × 9): each block's mean luma from 8 × 8 samples across it.
uniform sampler2D uFrame;

void main() {
  vec2 block = vec2(1.0 / 16.0, 1.0 / 9.0);
  vec2 corner = vTexCoord - 0.5 * block;
  float sum = 0.0;
  for (int row = 0; row < 8; row++) {
    for (int column = 0; column < 8; column++) {
      vec2 at = corner + (vec2(float(column), float(row)) + 0.5) * (block / 8.0);
      sum += luma(texture2D(uFrame, at).rgb);
    }
  }
  gl_FragColor = vec4(pack16(sum / 64.0), 0.0, 1.0);
}
