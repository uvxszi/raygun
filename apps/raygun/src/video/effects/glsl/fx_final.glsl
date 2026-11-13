// The picture as shown (drawn at the frame's size): sharpening (an unsharp mask over a 3 × 3 blur
// made of four bilinear taps between pixels, grain-sized differences left alone), the deflicker's
// gain unless denoising applied it, then the colour matrix (light, colour and look) and gamma.
uniform sampler2D uFrame;
uniform vec2 uTexel;
uniform float uDeflicker;
uniform float uSharpen;
uniform mat3 uColour;
uniform vec3 uOffset;
uniform float uGamma;
// The brightest a value may be: 1, or far above for HDR's linear light (passed through as is).
uniform float uLimit;

void main() {
  vec3 colour = texture2D(uFrame, vTexCoord).rgb;
  if (uSharpen > 0.0) {
    vec2 nudge = 0.5 * uTexel;
    vec3 blur = 0.25 * (
      texture2D(uFrame, vTexCoord + vec2(-nudge.x, -nudge.y)).rgb +
      texture2D(uFrame, vTexCoord + vec2(nudge.x, -nudge.y)).rgb +
      texture2D(uFrame, vTexCoord + vec2(-nudge.x, nudge.y)).rgb +
      texture2D(uFrame, vTexCoord + vec2(nudge.x, nudge.y)).rgb);
    vec3 detail = colour - blur;
    detail = sign(detail) * max(abs(detail) - 0.004, 0.0);
    colour += uSharpen * detail;
  }
  if (uDeflicker > 0.5) {
    colour *= flickerGain();
  }
  colour = clamp(uColour * colour + uOffset, 0.0, uLimit);
  if (uGamma != 1.0) {
    colour = pow(max(colour, vec3(0.00001)), vec3(uGamma));
  }
  gl_FragColor = vec4(colour, 1.0);
}
