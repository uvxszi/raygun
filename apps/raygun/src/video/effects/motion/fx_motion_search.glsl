// Smooth motion, step 2: block matching at one level of the luma pyramids, coarse to fine. Each
// pixel's vector is the motion from the frame before to this one, centred between them: the
// frame before is read at -v/2, this one at +v/2, so the field sits where in-between frames are
// drawn. The search starts from the coarser level's vector (or none), tries the offsets within
// `uRadius` steps and keeps the one whose 3 × 3 patch matches best, a step away costing a little.
uniform sampler2D uPrev;
uniform sampler2D uCur;
uniform sampler2D uInit;
// 1 when uInit holds the coarser level's flow.
uniform float uHasInit;
// This level's texel.
uniform vec2 uTexel;
// The finest level's pixels in one of this level's.
uniform float uScale;
// How far a step goes, in this level's pixels (1, or 0.5 to refine).
uniform float uStep;
// How many steps either way (at most 3).
uniform float uRadius;

const float SEARCH_PENALTY = 0.004;
const float ZERO_BIAS = 0.003;

float patchCost(vec2 flow) {
  vec2 halfway = flow * 0.5 * uTexel;
  float sum = 0.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 offset = vec2(float(x), float(y)) * uTexel;
      sum += abs(texture2D(uPrev, vTexCoord - halfway + offset).r - texture2D(uCur, vTexCoord + halfway + offset).r);
    }
  }
  return sum / 9.0;
}

void main() {
  vec2 start = uHasInit > 0.5 ? decodeFlow(texture2D(uInit, vTexCoord)) / uScale : vec2(0.0);
  vec2 best = start;
  float bestCost = patchCost(start);
  for (int y = -3; y <= 3; y++) {
    for (int x = -3; x <= 3; x++) {
      vec2 steps = vec2(float(x), float(y));
      if (max(abs(steps.x), abs(steps.y)) > uRadius || (x == 0 && y == 0)) continue;
      vec2 flow = start + steps * uStep;
      float cost = patchCost(flow) + SEARCH_PENALTY * length(steps * uStep);
      if (cost < bestCost) {
        best = flow;
        bestCost = cost;
      }
    }
  }
  // No motion wins a near tie: still backgrounds stay still.
  float still = patchCost(vec2(0.0));
  if (still <= bestCost + ZERO_BIAS) {
    best = vec2(0.0);
    bestCost = still;
  }
  gl_FragColor = encodeFlow(best * uScale, bestCost);
}
