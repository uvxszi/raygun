// The watermark remover (drawn at the frame's size, before the other passes): inside the box the
// overlay's blend I = a * W + (1 - a) * J is inverted with the opacity and colour estimated from
// the frames (uMask: W in rgb, a in alpha, rows from the box's top), parts too opaque to invert
// are filled from the box's edges, brightness and contrast correct what's left, and the box's
// edge blends into the picture over the feather. Outside the box the frame passes as it is.
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
  vec3 restored = (frame.rgb - alpha * mask.rgb) / max(1.0 - alpha, 0.08);
  // Too opaque to invert: the picture just outside the box, nearer edges counting more.
  float left = local.x + 0.02;
  float right = 1.02 - local.x;
  float bottom = local.y + 0.02;
  float top = 1.02 - local.y;
  vec3 fill =
    texture2D(uFrame, vec2(uBox.x - uTexel.x, vTexCoord.y)).rgb / left +
    texture2D(uFrame, vec2(uBox.z + uTexel.x, vTexCoord.y)).rgb / right +
    texture2D(uFrame, vec2(vTexCoord.x, uBox.y - uTexel.y)).rgb / bottom +
    texture2D(uFrame, vec2(vTexCoord.x, uBox.w + uTexel.y)).rgb / top;
  fill /= 1.0 / left + 1.0 / right + 1.0 / bottom + 1.0 / top;
  restored = mix(restored, fill, smoothstep(0.7, 0.92, alpha));
  // What's left of the overlay, corrected where it was.
  vec3 corrected = (restored - 0.5) * uContrast + 0.5 + uBright;
  restored = mix(restored, corrected, smoothstep(0.02, 0.12, alpha));
  float edge = min(min(local.x, 1.0 - local.x), min(local.y, 1.0 - local.y));
  float keep = uFeather > 0.0 ? smoothstep(0.0, uFeather, edge) : 1.0;
  gl_FragColor = vec4(mix(frame.rgb, clamp(restored, 0.0, 1.0), keep), frame.a);
}
