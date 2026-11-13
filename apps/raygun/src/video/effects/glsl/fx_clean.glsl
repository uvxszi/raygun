// Denoise (drawn at the frame's size): the deflicker's gain first, so still parts match from frame
// to frame; then neighbours weighted by how alike they are (edges stay); then the frame before,
// leaned on wherever little moved. The result is also the next frame's history.
uniform sampler2D uFrame;
uniform sampler2D uHistory;
uniform vec2 uTexel;
uniform float uDeflicker;
uniform float uSpatial;
uniform float uSpatialSigma;
// The frame before's most weight; 0 without one.
uniform float uTemporal;
uniform float uTemporalSigma;

void addNeighbour(vec2 offset, float gain, float centre, inout vec3 sum, inout float total) {
  vec3 colour = texture2D(uFrame, vTexCoord + offset * uTexel).rgb * gain;
  float difference = luma(colour) - centre;
  float weight = uSpatial * exp(-(difference * difference) / (2.0 * uSpatialSigma * uSpatialSigma));
  sum += colour * weight;
  total += weight;
}

void main() {
  float gain = uDeflicker > 0.5 ? flickerGain() : 1.0;
  vec3 colour = texture2D(uFrame, vTexCoord).rgb * gain;
  if (uSpatial > 0.0) {
    float centre = luma(colour);
    vec3 sum = colour;
    float total = 1.0;
    addNeighbour(vec2(1.0, 0.0), gain, centre, sum, total);
    addNeighbour(vec2(-1.0, 0.0), gain, centre, sum, total);
    addNeighbour(vec2(0.0, 1.0), gain, centre, sum, total);
    addNeighbour(vec2(0.0, -1.0), gain, centre, sum, total);
    addNeighbour(vec2(1.0, 1.0), gain, centre, sum, total);
    addNeighbour(vec2(-1.0, 1.0), gain, centre, sum, total);
    addNeighbour(vec2(1.0, -1.0), gain, centre, sum, total);
    addNeighbour(vec2(-1.0, -1.0), gain, centre, sum, total);
    colour = sum / total;
  }
  if (uTemporal > 0.0) {
    vec3 before = texture2D(uHistory, vTexCoord).rgb;
    vec3 change = abs(colour - before);
    float moved = max(change.r, max(change.g, change.b));
    float weight = uTemporal * exp(-(moved * moved) / (2.0 * uTemporalSigma * uTemporalSigma));
    colour = mix(colour, before, weight);
  }
  gl_FragColor = vec4(colour, 1.0);
}
