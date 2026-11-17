// Smooth motion: a frame copied as it is (kept for the next in-between frames, or sent on).
uniform sampler2D uCopyFrom;

void main() {
  gl_FragColor = texture2D(uCopyFrom, vTexCoord);
}
