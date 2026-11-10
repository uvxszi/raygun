import type { DeviceState, MotionLevel, PlaybackState } from '@raygun/bindings';
import { focusManager } from '@tanstack/react-query';
import { act, render, screen } from '@testing-library/react';
import type { FC } from 'react';

import { NowPlayingBars } from '../player/NowPlayingBars';
import { useLiveValue, ticking } from '../player/useLivePosition';
import { useApplyFeel } from '../settings/useApplyTheme';
import { bindingsMock, emitEvent, IDLE_STATE, seedBindings } from '../test/bindingsMock';
import { effectiveMotion, resetDeviceState, scrollMotion, useDeviceStateSync } from './deviceState';
import { offscreenWatched } from './usePauseOffscreen';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const ON_SCREEN: DeviceState = { foreground: true, powerSave: false, motion: 'full' };

const Feel: FC<{ motion?: MotionLevel }> = ({ motion = 'lively' }) => {
  useDeviceStateSync();
  const effective = useApplyFeel('thin', motion);
  return <span data-testid="motion">{effective}</span>;
};

const root = () => document.documentElement.dataset;

const setVisibility = (state: DocumentVisibilityState) => {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: state });
  document.dispatchEvent(new Event('visibilitychange'));
};

beforeEach(() => {
  seedBindings();
  resetDeviceState();
  setVisibility('visible');
});

describe('effectiveMotion', () => {
  it('lowers the setting by the system, battery saver and reduced motion', () => {
    expect(effectiveMotion('lively', ON_SCREEN, false)).toBe('lively');
    expect(effectiveMotion('calm', ON_SCREEN, false)).toBe('calm');
    expect(effectiveMotion('off', ON_SCREEN, false)).toBe('off');
    // Remove animations, or an animator scale of 0.
    expect(effectiveMotion('lively', { ...ON_SCREEN, motion: 'off' }, false)).toBe('off');
    // A scale of 0.5, battery saver, the browser's reduced motion.
    expect(effectiveMotion('lively', { ...ON_SCREEN, motion: 'reduced' }, false)).toBe('reduced');
    expect(effectiveMotion('calm', { ...ON_SCREEN, powerSave: true }, false)).toBe('reduced');
    expect(effectiveMotion('lively', ON_SCREEN, true)).toBe('reduced');
    // Off in Settings stays off whatever the system says.
    expect(effectiveMotion('off', { ...ON_SCREEN, motion: 'reduced' }, true)).toBe('off');
  });
});

describe('motion gating', () => {
  it("follows the device's animation settings and battery saver on the root", async () => {
    commands.deviceState.mockResolvedValue({ ...ON_SCREEN, motion: 'reduced' });
    render(<Feel />);
    await vi.waitFor(() => expect(root().motion).toBe('reduced'));
    expect(scrollMotion()).toBe('auto');

    act(() => emitEvent('deviceStateEvent', ON_SCREEN));
    expect(root().motion).toBe('lively');
    expect(scrollMotion()).toBe('smooth');

    act(() => emitEvent('deviceStateEvent', { ...ON_SCREEN, motion: 'off' }));
    expect(root().motion).toBe('off');
    expect(screen.getByTestId('motion')).toHaveTextContent('off');

    act(() => emitEvent('deviceStateEvent', { ...ON_SCREEN, powerSave: true }));
    expect(root().motion).toBe('reduced');
  });

  it('keeps Off from Settings when the system allows everything', async () => {
    render(<Feel motion="off" />);
    await vi.waitFor(() => expect(commands.deviceState).toHaveBeenCalled());
    expect(root().motion).toBe('off');
  });
});

describe('pausing out of sight', () => {
  it('marks the root inactive in the background and tells Rust when the page hides', async () => {
    render(<Feel />);
    await vi.waitFor(() => expect(root().active).toBe('true'));
    expect(focusManager.isFocused()).toBe(true);

    // Android: the activity stopped while the webview is held visible for YouTube.
    act(() => emitEvent('deviceStateEvent', { ...ON_SCREEN, foreground: false }));
    expect(root().active).toBe('false');
    // Polling queries (react-query's refetchInterval) stop.
    expect(focusManager.isFocused()).toBe(false);

    act(() => emitEvent('deviceStateEvent', ON_SCREEN));
    expect(root().active).toBe('true');
    expect(focusManager.isFocused()).toBe(true);

    // Desktop: a minimized window hides the page.
    act(() => setVisibility('hidden'));
    expect(commands.deviceSetVisible).toHaveBeenLastCalledWith(false);
    expect(root().active).toBe('false');
    act(() => setVisibility('visible'));
    expect(commands.deviceSetVisible).toHaveBeenLastCalledWith(true);
    expect(root().active).toBe('true');
  });

  it('says the page is visible on start before reading the state', async () => {
    // The page before (a reload, Android's window made again) said hidden on its way out.
    commands.deviceSetVisible.mockClear();
    commands.deviceState.mockResolvedValue(ON_SCREEN);
    render(<Feel />);
    await vi.waitFor(() => expect(commands.deviceState).toHaveBeenCalled());
    expect(commands.deviceSetVisible).toHaveBeenCalledWith(true);
    expect(commands.deviceSetVisible.mock.invocationCallOrder[0]).toBeLessThan(
      commands.deviceState.mock.invocationCallOrder[0] ?? 0,
    );
    await vi.waitFor(() => expect(root().active).toBe('true'));
  });

  it("stops the playhead's timer while Raygun is in the background", async () => {
    vi.useFakeTimers();
    try {
      const playing: PlaybackState = {
        ...IDLE_STATE,
        status: 'playing',
        positionMs: 1000,
        durationMs: 60_000,
      };
      const Playhead: FC = () => {
        useDeviceStateSync();
        const seconds = useLiveValue(playing, (position) => Math.floor(position / 1000));
        return <span>{seconds}</span>;
      };
      render(<Playhead />);
      await act(async () => {
        await Promise.resolve();
      });
      expect(ticking()).toBe(true);
      const onScreen = vi.getTimerCount();

      act(() => emitEvent('deviceStateEvent', { ...ON_SCREEN, foreground: false }));
      expect(ticking()).toBe(false);
      expect(vi.getTimerCount()).toBe(onScreen - 1);
      // Nothing wakes the page while it's away.
      act(() => vi.advanceTimersByTime(10_000));
      expect(ticking()).toBe(false);

      act(() => emitEvent('deviceStateEvent', ON_SCREEN));
      expect(ticking()).toBe(true);
    } finally {
      vi.useRealTimers();
    }
  });

  it('pauses the playing mark while it is scrolled away', () => {
    let report: IntersectionObserverCallback = () => {};
    class FakeObserver {
      constructor(callback: IntersectionObserverCallback) {
        report = callback;
      }
      observe = vi.fn();
      unobserve = vi.fn();
      disconnect = vi.fn();
    }
    vi.stubGlobal('IntersectionObserver', FakeObserver);
    try {
      const { unmount } = render(<NowPlayingBars playing />);
      const mark = screen.getByTestId('now-playing-bars');
      expect(offscreenWatched()).toBe(1);
      const seen = (isIntersecting: boolean) =>
        report(
          [{ target: mark, isIntersecting } as unknown as IntersectionObserverEntry],
          {} as IntersectionObserver,
        );
      act(() => seen(false));
      expect(mark).toHaveAttribute('data-offscreen');
      act(() => seen(true));
      expect(mark).not.toHaveAttribute('data-offscreen');
      unmount();
      expect(offscreenWatched()).toBe(0);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
