// Smooth motion, step 3: the flow's vector median over 3 × 3 (the neighbour's vector closest to
// all the others), which drops lone wrong matches and keeps the edges between moving things.
uniform sampler2D uFlowIn;
uniform vec2 uTexel;

void main() {
  vec4 stored[9];
  vec2 flows[9];
  for (int i = 0; i < 9; i++) {
    vec2 at = vec2(float(i - (i / 3) * 3 - 1), float(i / 3 - 1));
    stored[i] = texture2D(uFlowIn, vTexCoord + at * uTexel);
    flows[i] = decodeFlow(stored[i]);
  }
  vec4 chosen = stored[4];
  float least = 1.0e9;
  for (int i = 0; i < 9; i++) {
    float spread = 0.0;
    for (int j = 0; j < 9; j++) {
      spread += length(flows[i] - flows[j]);
    }
    if (spread < least) {
      least = spread;
      chosen = stored[i];
    }
  }
  gl_FragColor = chosen;
}
