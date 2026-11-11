import type { PlaybackState } from '@raygun/bindings';

import { IDLE_STATE } from '../test/bindingsMock';
import { PRESS_HOLD_MS, pressPlay, pressWaiting, receivePlayerState } from './playPress';

const state = (status: PlaybackState['status'], extra: Partial<PlaybackState> = {}) => ({
  ...IDLE_STATE,
  currentItemId: 'song',
  positionMs: 30_000,
  durationMs: 180_000,
  status,
  ...extra,
});

describe('A press of Play or Pause', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows what it asks for at once, never waiting', () => {
    const show = vi.fn();
    pressPlay('here', state('paused', { waiting: false }), true, show, vi.fn());
    expect(show).toHaveBeenCalledWith(
      expect.objectContaining({ status: 'loading', waiting: false }),
    );

    pressPlay('here', state('playing', { waiting: true }), false, show, vi.fn());
    expect(show).toHaveBeenLastCalledWith(
      expect.objectContaining({ status: 'paused', waiting: false, positionMs: 30_000 }),
    );
  });

  it('keeps a report sent before it off screen until Rust answers', () => {
    pressPlay('here', state('playing'), false, vi.fn(), vi.fn());

    // A position report from before the press still says it plays.
    const stale = vi.fn();
    receivePlayerState('here', state('playing'), stale);
    expect(stale).not.toHaveBeenCalled();

    // Rust's answer agrees: it shows, and the press is over.
    const answer = vi.fn();
    receivePlayerState('here', state('paused'), answer);
    expect(answer).toHaveBeenCalledOnce();
    expect(pressWaiting()).toBe(false);
    vi.advanceTimersByTime(PRESS_HOLD_MS);
    expect(stale).not.toHaveBeenCalled();

    // Later word shows at once.
    const later = vi.fn();
    receivePlayerState('here', state('playing'), later);
    expect(later).toHaveBeenCalledOnce();
  });

  it("shows Rust's word that says otherwise once it has held long enough", () => {
    const ask = vi.fn();
    pressPlay('here', state('paused'), true, vi.fn(), ask);
    const otherwise = vi.fn();
    receivePlayerState('here', state('paused', { positionMs: 31_000 }), otherwise);
    expect(otherwise).not.toHaveBeenCalled();

    vi.advanceTimersByTime(PRESS_HOLD_MS);
    expect(otherwise).toHaveBeenCalledOnce();
    expect(ask).not.toHaveBeenCalled();
  });

  it('asks Rust again when it says nothing', () => {
    const ask = vi.fn();
    pressPlay('here', state('playing'), false, vi.fn(), ask);
    vi.advanceTimersByTime(PRESS_HOLD_MS - 1);
    expect(ask).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(ask).toHaveBeenCalledOnce();
    expect(pressWaiting()).toBe(false);
  });

  it('lets news of another song or another player through', () => {
    pressPlay('here', state('playing'), false, vi.fn(), vi.fn());

    // Connect's player isn't the one pressed.
    const connected = vi.fn();
    receivePlayerState('connected', state('playing'), connected);
    expect(connected).toHaveBeenCalledOnce();
    expect(pressWaiting()).toBe(true);

    // The song ended meanwhile and the next one plays: that shows, and the press is over.
    const next = vi.fn();
    receivePlayerState('here', state('loading', { currentItemId: 'next' }), next);
    expect(next).toHaveBeenCalledOnce();
    expect(pressWaiting()).toBe(false);
  });

  it("keeps Rust's word between two presses for when they're over", () => {
    const ask = vi.fn();
    pressPlay('here', state('playing'), false, vi.fn(), ask);
    const between = vi.fn();
    receivePlayerState('here', state('playing'), between);
    // Pressed again before Rust answered the first: it asks for playing.
    pressPlay('here', state('paused'), true, vi.fn(), vi.fn());
    // Rust's answer to the first press says paused: not what the second asked for.
    const first = vi.fn();
    receivePlayerState('here', state('paused'), first);
    expect(first).not.toHaveBeenCalled();
    // Its answer to the second agrees.
    const second = vi.fn();
    receivePlayerState('here', state('playing'), second);
    expect(second).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(PRESS_HOLD_MS);
    expect(ask).not.toHaveBeenCalled();
    expect(between).not.toHaveBeenCalled();
  });
});
