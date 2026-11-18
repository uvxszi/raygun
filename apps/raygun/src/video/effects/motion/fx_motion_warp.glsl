// Smooth motion, step 5: an in-between frame at uT (0 the frame before, 1 the next), by the model
// chosen (uModel: 0 blend, 1 balanced, 2 sharp; the last two follow the flow alike).
//
// Blend: no flow. Where the two frames are alike (barely anything moved) they're blended by uT;
// elsewhere the nearer frame repeats, so nothing doubles.
//
// Balanced and sharp: each pixel follows the flow back to the frame before (by uT) and on to the
// next (by 1 - uT). A side counts only when its sample lies inside the picture (no stretched
// edges) and the flow where it lands agrees with this pixel's (forward/backward consistency: where
// a moving thing covers or uncovers the background, one side lands on the other motion and is left
// out, the other side drawing alone). Where the two samples still disagree, the nearer one is kept.
// A block that matched badly, a trajectory crossing a motion edge, or neither side counting keeps
// the nearer frame as it is (a frame repeat), and after a cut the nearer frame shows as it is. The
// same sums are `warpPixel` in motionMath.ts and MotionMath, whose tests check them.
uniform sampler2D uPrev;
uniform sampler2D uCur;
uniform sampler2D uFlow;
uniform sampler2D uError;
uniform float uT;
uniform float uModel;
// Texture coordinates in one of the flow's pixels.
uniform vec2 uFlowTexel;

// The same as MotionMath's.
const float CUT_COST = 0.09;
const float CUT_SHARE = 0.45;
const float TRUST_FROM = 0.05;
const float TRUST_TO = 0.18;
const float BLEND_STILL = 0.03;
const float UNSURE_FROM = 0.1;
const float UNSURE_TO = 0.2;
const float AGREE_FROM = 1.0;
const float AGREE_TO = 3.0;
const float MAX_MOTION = 40.0;

float inside(vec2 at) {
  return step(0.0, at.x) * step(0.0, at.y) * step(at.x, 1.0) * step(at.y, 1.0);
}

float agrees(vec2 flow, vec2 at) {
  return 1.0 - smoothstep(AGREE_FROM, AGREE_TO, length(decodeFlow(texture2D(uFlow, at)) - flow));
}

void main() {
  vec4 here0 = texture2D(uPrev, vTexCoord);
  vec4 here1 = texture2D(uCur, vTexCoord);
  vec4 nearer = uT < 0.5 ? here0 : here1;
  if (uModel < 0.5) {
    float still = 1.0 - smoothstep(BLEND_STILL, 2.0 * BLEND_STILL, abs(luma(here0.rgb) - luma(here1.rgb)));
    gl_FragColor = mix(nearer, mix(here0, here1, uT), still);
    return;
  }
  vec4 error = texture2D(uError, vec2(0.5));
  if (error.r * COST_RANGE > CUT_COST || error.g > CUT_SHARE) {
    gl_FragColor = nearer;
    return;
  }
  vec4 stored = texture2D(uFlow, vTexCoord);
  vec2 flow = decodeFlow(stored);
  float size = length(flow);
  if (size > MAX_MOTION) flow *= MAX_MOTION / size;
  vec2 motion = flow * uFlowTexel;
  vec2 fromAt = vTexCoord - uT * motion;
  vec2 toAt = vTexCoord + (1.0 - uT) * motion;
  vec4 before = texture2D(uPrev, fromAt);
  vec4 after = texture2D(uCur, toAt);
  // The trajectory through this pixel is the flow at its midpoint; each side, the flow where its
  // sample lands.
  float path = agrees(flow, vTexCoord + (0.5 - uT) * motion);
  float weightBefore = (1.0 - uT) * inside(fromAt) * agrees(flow, fromAt);
  float weightAfter = uT * inside(toAt) * agrees(flow, toAt);
  float weights = weightBefore + weightAfter;
  vec4 warped = weights > 0.0 ? (before * weightBefore + after * weightAfter) / weights : nearer;
  // Both sides there but unlike each other: the nearer one alone.
  float differ = abs(luma(before.rgb) - luma(after.rgb));
  float both = min(weightBefore, weightAfter) > 0.0 ? 1.0 - smoothstep(TRUST_FROM, TRUST_TO, differ) : 1.0;
  vec4 nearerSide = weightBefore >= weightAfter ? before : after;
  if (weightBefore > 0.0 && weightAfter > 0.0) nearerSide = uT < 0.5 ? before : after;
  warped = mix(nearerSide, warped, both);
  // A badly matched block is believed where its two samples still agree (a patch across a moving
  // thing's edge matches badly, its own pixel well).
  float agreed = min(weightBefore, weightAfter) > 0.0 ? 1.0 - smoothstep(TRUST_FROM, TRUST_TO, differ) : 0.0;
  float sure = path * max(1.0 - smoothstep(UNSURE_FROM, UNSURE_TO, decodeCost(stored)), agreed) * step(0.05, weights);
  gl_FragColor = mix(nearer, warped, sure);
}
