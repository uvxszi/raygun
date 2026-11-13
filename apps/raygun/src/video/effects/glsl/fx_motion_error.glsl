// Smooth motion, step 4 (drawn 1 × 1): how well the two frames matched over the whole picture,
// from 16 × 9 of the flow's pixels: the mean cost (r, over COST_RANGE) and the share matched
// badly (g). A cut between scenes matches badly almost everywhere: the in-between frames then
// repeat the nearer frame (fx_motion_warp.glsl).
uniform sampler2D uCosts;

const float BAD_COST = 0.12;

void main() {
  float sum = 0.0;
  float bad = 0.0;
  for (int row = 0; row < 9; row++) {
    for (int column = 0; column < 16; column++) {
      vec2 at = (vec2(float(column), float(row)) + 0.5) / vec2(16.0, 9.0);
      float cost = decodeCost(texture2D(uCosts, at));
      sum += cost;
      bad += cost > BAD_COST ? 1.0 : 0.0;
    }
  }
  gl_FragColor = vec4(sum / 144.0 / COST_RANGE, bad / 144.0, 0.0, 1.0);
}
