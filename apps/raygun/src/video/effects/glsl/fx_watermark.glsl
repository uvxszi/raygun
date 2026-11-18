// The watermark remover (drawn at the frame's size, before the other passes): inside the box the
// overlay's blend I = a * W + (1 - a) * J is inverted with the opacity and colour estimated from
// the frames (uMask: a * W in rgb, a in alpha, rows from the box's top; premultiplied, so the
// texture's filtering between cells stays right). The estimate keeps a at most 0.6 and W near
// grey, and each channel of the result stays within 0..1, so the inversion can't tint the picture
// (`restorePixel` in watermarkMath.ts is the same sum). Brightness and contrast correct what's
// left where the overlay was, and the box's edge blends into the picture over the feather.
// Outside the box the frame passes as it is.
uniform sampler2D uFrame;
uniform sampler2D uMask;
uniform vec2 uTexel;
// The box in texture coordinates: x0, y0, x1, y1 (origin at the bottom left).
uniform vec4 uBox;
uniform float uFeather;
uniform float uBright;
uniform float uContrast;

void main() {
  vec4 frame = texture2D(uFrame, vTexCoord);
  vec2 size = uBox.zw - uBox.xy;
  vec2 local = (vTexCoord - uBox.xy) / size;
  if (local.x < 0.0 || local.y < 0.0 || local.x > 1.0 || local.y > 1.0) {
    gl_FragColor = frame;
    return;
  }
  vec4 mask = texture2D(uMask, vec2(local.x, 1.0 - local.y));
  float alpha = mask.a;
  vec3 restored = clamp((frame.rgb - mask.rgb) / max(1.0 - alpha, 0.4), 0.0, 1.0);
  // What's left of the overlay, corrected where it was.
  vec3 corrected = clamp((restored - 0.5) * uContrast + 0.5 + uBright, 0.0, 1.0);
  restored = mix(restored, corrected, smoothstep(0.05, 0.15, alpha));
  float edge = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y));
  float keep = uFeather > 0.0 ? smoothstep(0.0, uFeather, edge) : 1.0;
  gl_FragColor = vec4(mix(frame.rgb, restored, keep), frame.a);
}
