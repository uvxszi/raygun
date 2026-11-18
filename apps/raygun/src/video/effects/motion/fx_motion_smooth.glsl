// Smooth motion, step 3: the flow made steady. First the vector median over 3 × 3 (the
// neighbour's vector closest to all the others, each also judged by how well it carries this very
// pixel from one frame to the other, so a patch across a moving thing's edge takes the side the
// pixel is on), which drops lone wrong matches; then an
// edge-aware mean round it: neighbours count by how alike their picture is (motion edges follow
// the picture's edges), how near their vector is to the median and how well they matched, so
// vectors don't bleed across the edge of a moving thing. Vectors longer than MAX_MOTION are cut
// to it (no match that far is believed). The cost kept is the pixel's own, for the warp's
// confidence. The same numbers are `smoothFlow` in motionMath.ts and MotionMath.
uniform sampler2D uFlowIn;
// The two frames' finest luma levels (the flow's size).
uniform sampler2D uPrevLuma;
uniform sampler2D uCurLuma;
uniform vec2 uTexel;

const float MAX_MOTION = 40.0;
const float EDGE_SIGMA = 0.06;
const float VECTOR_SIGMA = 2.0;
const float COST_SIGMA = 0.08;
// How much a vector's spread from the others counts against how well it carries the pixel.
const float SPREAD_WEIGHT = 0.02;

float pictureAt(vec2 at) {
  return 0.5 * (texture2D(uPrevLuma, at).r + texture2D(uCurLuma, at).r);
}

void main() {
  vec2 flows[9];
  float costs[9];
  float pictures[9];
  for (int i = 0; i < 9; i++) {
    vec2 at = vTexCoord + vec2(float(i - (i / 3) * 3 - 1), float(i / 3 - 1)) * uTexel;
    vec4 stored = texture2D(uFlowIn, at);
    flows[i] = decodeFlow(stored);
    costs[i] = decodeCost(stored);
    pictures[i] = pictureAt(at);
  }
  vec2 median = flows[4];
  float least = 1.0e9;
  for (int i = 0; i < 9; i++) {
    float spread = 0.0;
    for (int j = 0; j < 9; j++) {
      spread += length(flows[i] - flows[j]);
    }
    vec2 halfway = flows[i] * 0.5 * uTexel;
    float carried = abs(texture2D(uPrevLuma, vTexCoord - halfway).r - texture2D(uCurLuma, vTexCoord + halfway).r);
    float score = spread / 9.0 * SPREAD_WEIGHT + carried;
    if (score < least) {
      least = score;
      median = flows[i];
    }
  }
  vec2 sum = vec2(0.0);
  float weights = 0.0;
  for (int i = 0; i < 9; i++) {
    float picture = (pictures[i] - pictures[4]) / EDGE_SIGMA;
    float vector = length(flows[i] - median) / VECTOR_SIGMA;
    float cost = costs[i] / COST_SIGMA;
    float weight = exp(-(picture * picture) - (vector * vector) - cost);
    sum += flows[i] * weight;
    weights += weight;
  }
  vec2 flow = weights > 1.0e-4 ? sum / weights : median;
  float size = length(flow);
  if (size > MAX_MOTION) flow *= MAX_MOTION / size;
  gl_FragColor = encodeFlow(flow, costs[4]);
}
