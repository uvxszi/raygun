import type { PlaybackState } from '@raygun/bindings';

/** Whether something plays (or is about to): what Play/Pause shows. */
export const isActive = (state: PlaybackState | undefined) =>
  state?.status === 'playing' || state?.status === 'loading' || state?.status === 'buffering';

/** Which player a press was about: this device's, or the one it plays on (Connect, M46.15). */
export type PressedPlayer = 'here' | 'connected';

/**
 * How long a press may keep Rust's word that says otherwise off screen, ms. Rust answers a press
 * well within it; only a word Rust never takes back (the player wasn't as shown) waits this long.
 */
export const PRESS_HOLD_MS = 1500;

type Press = {
  player: PressedPlayer;
  /** Whether it asked for playing. */
  active: boolean;
  /** The item it was about. */
  itemId: string | null;
  /** Puts Rust's latest word on screen once the press is over. */
  settle: () => void;
  timer: ReturnType<typeof setTimeout>;
};

let press: Press | null = null;

const finish = () => {
  if (!press) return;
  const { settle, timer } = press;
  clearTimeout(timer);
  press = null;
  settle();
};

/**
 * What a press shows until Rust answers: paused, or about to play. Resuming shows no wait: Rust
 * says when a stream really waits.
 */
export const pressedState = (state: PlaybackState, active: boolean): PlaybackState => ({
  ...state,
  status: active ? 'loading' : 'paused',
  waiting: false,
});

/**
 * Play or Pause pressed: `show` puts what it asks for on screen at once, before Rust hears of it,
 * so the button flips on the tap. `state` is the shown state, its position where playback is now.
 * `ask` gets Rust's word again, should it say nothing meanwhile. A press on a press keeps what
 * Rust said in between.
 */
export const pressPlay = (
  player: PressedPlayer,
  state: PlaybackState,
  active: boolean,
  show: (state: PlaybackState) => void,
  ask: () => void,
) => {
  const previous = press?.player === player ? press : null;
  if (press) clearTimeout(press.timer);
  press = {
    player,
    active,
    itemId: state.currentItemId,
    settle: previous?.settle ?? ask,
    timer: setTimeout(finish, PRESS_HOLD_MS),
  };
  show(pressedState(state, active));
};

/**
 * Rust's word about `player`, before it shows: at once, unless a press waits for Rust's answer
 * and this says otherwise about the same song (a report sent before the press reached Rust).
 * That one shows when the press is over: once Rust's answer agrees, or after
 * {@link PRESS_HOLD_MS}.
 */
export const receivePlayerState = (
  player: PressedPlayer,
  state: PlaybackState | undefined,
  show: () => void,
) => {
  if (!press || press.player !== player) {
    show();
    return;
  }
  const otherwise =
    state !== undefined && state.currentItemId === press.itemId && isActive(state) !== press.active;
  if (otherwise) {
    press.settle = show;
    return;
  }
  clearTimeout(press.timer);
  press = null;
  show();
};

/** Whether a press waits for Rust's answer (tests). */
export const pressWaiting = () => press !== null;

/** Forgets a press without showing anything (tests). */
export const forgetPress = () => {
  if (press) clearTimeout(press.timer);
  press = null;
};
