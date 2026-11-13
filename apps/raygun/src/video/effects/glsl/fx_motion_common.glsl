// Smooth motion's shared part (GLSL ES 1.00), put after fx_common.glsl in each motion pass. The
// flow between two frames is kept in RGBA8 textures: a vector in the flow's own pixels (its finest
// level) as rg, 128 meaning no motion (so none is exact), and how badly its patches matched as b.
// The same numbers are in PlayerEffectsMotionMath.kt (`MotionMath`) and motionMath.ts, whose tests
// check them.

const float FLOW_RANGE = 64.0;
const float COST_RANGE = 0.5;
const float FLOW_ZERO = 128.0 / 255.0;

vec2 decodeFlow(vec4 stored) {
  return (stored.rg - FLOW_ZERO) * FLOW_RANGE;
}

vec4 encodeFlow(vec2 flow, float cost) {
  return vec4(clamp(flow / FLOW_RANGE + FLOW_ZERO, 0.0, 1.0), clamp(cost / COST_RANGE, 0.0, 1.0), 1.0);
}

float decodeCost(vec4 stored) {
  return stored.b * COST_RANGE;
}
