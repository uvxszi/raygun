/**
 * Moves a scrolling box to a line the way Apple Music's lyrics move: a spring that doesn't bounce
 * (critically damped). A new target while it moves bends the same move toward it, at the speed it
 * has, instead of starting over or stopping short. Only `scrollTop` changes, once a frame, and only
 * while it moves.
 */

/** How quickly a glide closes in, per second: 90 % of the way in about 0.35 s, still by 0.6 s. */
const LIVELY = 11;
/** Calm moves less: the same glide, done sooner. */
const CALM = 14;
/** Close enough to stop: px, and px per second. */
const REST_PX = 0.5;
const REST_SPEED = 8;
/** A longer frame (a busy or hidden page) counts as this long, s, so nothing leaps. */
const MAX_STEP_S = 0.064;
/** The box is this far from where the glide put it: the listener (or the page) scrolled it. */
const TAKEN_OVER_PX = 2;

export type Glide = {
  /** Moves toward `top`, kept within what the box scrolls: eased, or at once with `instant`. */
  to: (top: number, instant?: boolean) => void;
  /** Stops where it is. */
  stop: () => void;
};

/**
 * How quickly things glide now (`data-motion` on the root, shell/deviceState): `null` where they
 * may not move (Reduced, Off), so the box goes straight to the line.
 */
export const glideRate = (): number | null => {
  const level = document.documentElement.dataset.motion;
  if (level === 'reduced' || level === 'off') return null;
  return level === 'calm' ? CALM : LIVELY;
};

/**
 * One step of the spring toward 0 from `offset` at `velocity` over `dt` seconds (exact, so it
 * moves the same at any frame rate): the new offset and velocity.
 */
export const springStep = (offset: number, velocity: number, rate: number, dt: number) => {
  const decay = Math.exp(-rate * dt);
  const lean = velocity + rate * offset;
  return {
    offset: (offset + lean * dt) * decay,
    velocity: (velocity - rate * lean * dt) * decay,
  };
};

export const createGlide = (box: HTMLElement): Glide => {
  let frame = 0;
  let position = 0;
  let velocity = 0;
  let target = 0;
  let last = 0;
  let written = 0;
  let rate = LIVELY;

  const write = (top: number) => {
    box.scrollTop = top;
    written = box.scrollTop;
  };

  const stop = () => {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    velocity = 0;
  };

  const step = (now: number) => {
    frame = 0;
    // Scrolled meanwhile by someone else: they win.
    if (Math.abs(box.scrollTop - written) > TAKEN_OVER_PX) {
      velocity = 0;
      return;
    }
    const dt = Math.min(MAX_STEP_S, Math.max(0, (now - last) / 1000));
    last = now;
    const next = springStep(position - target, velocity, rate, dt);
    velocity = next.velocity;
    if (Math.abs(next.offset) < REST_PX && Math.abs(velocity) < REST_SPEED) {
      position = target;
      velocity = 0;
      write(position);
      return;
    }
    position = target + next.offset;
    write(position);
    frame = requestAnimationFrame(step);
  };

  return {
    to(top, instant = false) {
      const most = Math.max(0, box.scrollHeight - box.clientHeight);
      target = Math.min(most, Math.max(0, top));
      const glides = glideRate();
      if (instant || glides === null || typeof requestAnimationFrame !== 'function') {
        stop();
        position = target;
        write(target);
        return;
      }
      rate = glides;
      // Moving already: the same move now bends toward the new target.
      if (frame) return;
      position = box.scrollTop;
      written = position;
      velocity = 0;
      if (Math.abs(position - target) < REST_PX) return;
      last = performance.now();
      frame = requestAnimationFrame(step);
    },
    stop,
  };
};
