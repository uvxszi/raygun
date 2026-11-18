// Smooth motion, step 2: block matching at one level of the luma pyramids, coarse to fine. Each
// pixel's vector is the motion from the frame before to this one, centred between them: the
// frame before is read at -v/2, this one at +v/2, so the field sits where in-between frames are
// drawn. The search starts from the best of the coarser level's vector here, its neighbours' (a
// moving thing's edge takes the motion of the side it's on, not the coarse blur of both) and no
// motion, then tries the offsets within `uRadius` steps and keeps the one whose 3 × 3 patch
// matches best, a step away costing a little. Taps falling outside either frame don't count (the
// picture's edge has nothing to match; new picture coming in matches by what's inside). The same
// sums are `search` and `flowField` in motionMath.ts and MotionMath.
uniform sampler2D uPrev;
uniform sampler2D uCur;
uniform sampler2D uInit;
// 1 when uInit holds the coarser level's flow.
uniform float uHasInit;
// This level's texel.
uniform vec2 uTexel;
// The finest level's pixels in one of this level's.
uniform float uScale;
// How far a step goes, in this level's pixels (1, or less to refine).
uniform float uStep;
// How many steps either way (at most 3).
uniform float uRadius;

const float SEARCH_PENALTY = 0.004;
const float ZERO_BIAS = 0.003;
// A patch with fewer taps inside both frames than this matches nothing.
const float FEWEST_TAPS = 3.0;
const float NO_MATCH = 1.0;

float inside(vec2 at) {
  return step(0.0, at.x) * step(0.0, at.y) * step(at.x, 1.0) * step(at.y, 1.0);
}

float patchCost(vec2 flow) {
  vec2 halfway = flow * 0.5 * uTexel;
  float sum = 0.0;
  float taps = 0.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      vec2 offset = vec2(float(x), float(y)) * uTexel;
      vec2 before = vTexCoord - halfway + offset;
      vec2 after = vTexCoord + halfway + offset;
      float counted = inside(before) * inside(after);
      sum += counted * abs(texture2D(uPrev, before).r - texture2D(uCur, after).r);
      taps += counted;
    }
  }
  return taps < FEWEST_TAPS ? NO_MATCH : sum / taps;
}

vec2 initAt(vec2 offset) {
  return decodeFlow(texture2D(uInit, vTexCoord + offset * uTexel * 2.0)) / uScale;
}

void main() {
  vec2 start = vec2(0.0);
  float startCost = patchCost(start);
  if (uHasInit > 0.5) {
    for (int i = 0; i < 5; i++) {
      vec2 offset = i == 0 ? vec2(0.0) : i == 1 ? vec2(-1.0, 0.0) : i == 2 ? vec2(1.0, 0.0) : i == 3 ? vec2(0.0, -1.0) : vec2(0.0, 1.0);
      vec2 candidate = initAt(offset);
      float cost = patchCost(candidate);
      if (cost < startCost) {
        start = candidate;
        startCost = cost;
      }
    }
  }
  vec2 best = start;
  float bestCost = startCost;
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
