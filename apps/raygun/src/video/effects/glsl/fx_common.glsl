// Put in front of each effect pass's fragment shader (GLSL ES 1.00), on Android and desktop.
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 vTexCoord;

// The deflicker's light: this frame's mean luma (rg) and the steady mean it's held to (ba).
uniform sampler2D uStats;

// Rec. 709 luma.
float luma(vec3 rgb) {
  return dot(rgb, vec3(0.2126, 0.7152, 0.0722));
}

// A value from 0 to 1 in two 8-bit channels, so means keep 16 bits in an RGBA8 texture.
vec2 pack16(float value) {
  float scaled = clamp(value, 0.0, 1.0) * 255.0;
  float high = floor(scaled);
  return vec2(high / 255.0, scaled - high);
}

float unpack16(vec2 stored) {
  return (floor(stored.x * 255.0 + 0.5) + stored.y) / 255.0;
}

// The deflicker's gain for this frame: the steady light over its own, within limits (a tenth of
// a step of light in dark frames counts for less, so their noise isn't chased).
float flickerGain() {
  vec4 light = texture2D(uStats, vec2(0.5));
  float mean = unpack16(light.rg);
  float steady = unpack16(light.ba);
  return clamp((steady + 0.01) / (mean + 0.01), 0.75, 1.33);
}
