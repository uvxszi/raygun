// Smooth motion, step 5: an in-between frame at uT (0 the frame before, 1 the next). Each pixel
// follows the flow back to the frame before (by uT) and on to the next (by 1 - uT) and blends the
// two by uT. Where they disagree (something appeared, or the match was poor) the nearer frame's
// sample is kept alone; after a cut the nearer frame shows as it is (the frame repeats).
uniform sampler2D uPrev;
uniform sampler2D uCur;
uniform sampler2D uFlow;
uniform sampler2D uError;
uniform float uT;
// Texture coordinates in one of the flow's pixels.
uniform vec2 uFlowTexel;

// The same as MotionMath's CUT_COST, CUT_SHARE, TRUST_FROM and TRUST_TO.
const float CUT_COST = 0.09;
const float CUT_SHARE = 0.45;
const float TRUST_FROM = 0.05;
const float TRUST_TO = 0.18;

void main() {
  vec4 error = texture2D(uError, vec2(0.5));
  if (error.r * COST_RANGE > CUT_COST || error.g > CUT_SHARE) {
    gl_FragColor = uT < 0.5 ? texture2D(uPrev, vTexCoord) : texture2D(uCur, vTexCoord);
    return;
  }
  vec4 stored = texture2D(uFlow, vTexCoord);
  vec2 motion = decodeFlow(stored) * uFlowTexel;
  vec4 before = texture2D(uPrev, vTexCoord - uT * motion);
  vec4 after = texture2D(uCur, vTexCoord + (1.0 - uT) * motion);
  float differ = max(abs(luma(before.rgb) - luma(after.rgb)), decodeCost(stored));
  float trust = 1.0 - smoothstep(TRUST_FROM, TRUST_TO, differ);
  vec4 nearer = uT < 0.5 ? before : after;
  gl_FragColor = mix(nearer, mix(before, after, uT), trust);
}
