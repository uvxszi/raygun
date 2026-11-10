import { commands, events } from '@raygun/bindings';
import type { DeviceState, MotionLevel } from '@raygun/bindings';
import { focusManager } from '@tanstack/react-query';
import { useEffect, useSyncExternalStore } from 'react';

/**
 * What the device lets Raygun do right now (Rust's `device_state`): whether it's on screen,
 * battery saver, and the system's animation settings. The page's own visibility counts too (on
 * Android the webview can be kept "visible" while Raygun is in the background, so it's never the
 * only signal).
 *
 * While Raygun isn't active, polling stops (react-query's focus follows `isAppActive`), the live
 * playhead stops ticking and CSS animations pause (`data-active` on the root).
 */

/** How much things move on screen: the setting, lowered by the system and battery saver. */
export type EffectiveMotion = 'lively' | 'calm' | 'reduced' | 'off';

const START: DeviceState = { foreground: true, powerSave: false, motion: 'full' };

let device: DeviceState = START;
let pageVisible = typeof document === 'undefined' || document.visibilityState !== 'hidden';
let prefersReduced = false;
const listeners = new Set<() => void>();

const notify = () => {
  for (const listener of [...listeners]) listener();
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** Raygun is on screen: the device says so and the page is visible. */
export const isAppActive = () => device.foreground && pageVisible;

/** Follows [isAppActive]; `listener` gets the new value when it changes. */
export const onAppActiveChange = (listener: (active: boolean) => void) => {
  let last = isAppActive();
  return subscribe(() => {
    const now = isAppActive();
    if (now !== last) {
      last = now;
      listener(now);
    }
  });
};

export const getDeviceState = () => device;

/** For tests and the event: replaces the device's state. */
export const setDeviceState = (next: DeviceState) => {
  if (
    next.foreground === device.foreground &&
    next.powerSave === device.powerSave &&
    next.motion === device.motion
  ) {
    return;
  }
  device = next;
  notify();
};

/** For tests: the page's visibility as `visibilitychange` reports it. */
export const setPageVisible = (visible: boolean) => {
  if (visible === pageVisible) return;
  pageVisible = visible;
  notify();
};

/** For tests: puts everything back as at start. */
export const resetDeviceState = () => {
  device = START;
  pageVisible = true;
  prefersReduced = false;
  notify();
};

/**
 * The motion level the page uses: Off in Settings or from the system (Remove animations, an
 * animator scale of 0) stops everything; the system's slower scales, battery saver and the
 * browser's reduced motion keep only short fades; otherwise the setting.
 */
export const effectiveMotion = (
  setting: MotionLevel | undefined,
  state: DeviceState,
  reducedByBrowser: boolean,
): EffectiveMotion => {
  if (setting === 'off' || state.motion === 'off') return 'off';
  if (state.motion === 'reduced' || state.powerSave || reducedByBrowser) return 'reduced';
  return setting ?? 'lively';
};

/** Whether anything may move, at all (`reduced` keeps only short fades). */
export const movesFreely = (motion: EffectiveMotion) => motion === 'lively' || motion === 'calm';

/**
 * How a script-driven scroll moves now: smoothly where things may move, at once otherwise (an
 * explicit `behavior` wins over the stylesheet's `scroll-behavior`).
 */
export const scrollMotion = (): ScrollBehavior => {
  const level = document.documentElement.dataset.motion;
  return level === 'reduced' || level === 'off' ? 'auto' : 'smooth';
};

type Snapshot = { device: DeviceState; active: boolean; prefersReduced: boolean };
let snapshot: Snapshot = { device, active: isAppActive(), prefersReduced };
const getSnapshot = () => {
  const active = isAppActive();
  if (
    snapshot.device !== device ||
    snapshot.active !== active ||
    snapshot.prefersReduced !== prefersReduced
  ) {
    snapshot = { device, active, prefersReduced };
  }
  return snapshot;
};

/** The device's state and whether Raygun is active, re-rendering when either changes. */
export const useDeviceState = () => useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

/** Raygun is on screen. */
export const useAppActive = () => useDeviceState().active;

/** The motion level in effect for `setting`. */
export const useEffectiveMotion = (setting: MotionLevel | undefined) => {
  const { device: state, prefersReduced: reduced } = useDeviceState();
  return effectiveMotion(setting, state, reduced);
};

/**
 * Starts following the device's state (once, from the shell): Rust's state and event, the page's
 * visibility (told to Rust: desktop has no other signal), the browser's reduced motion, and
 * react-query's focus, so polling queries stop while Raygun isn't active.
 */
export const useDeviceStateSync = () => {
  useEffect(() => {
    let alive = true;
    const unlisten = events.deviceStateEvent.listen((event) => setDeviceState(event.payload));

    const onVisibility = () => {
      const visible = document.visibilityState !== 'hidden';
      setPageVisible(visible);
      return commands.deviceSetVisible(visible).catch(() => {});
    };
    document.addEventListener('visibilitychange', onVisibility);
    // The page says how it is first: Rust keeps the last page's word, and a page unloading (a
    // reload, Android's window made again) says hidden on its way out. Then Rust's state is read.
    void onVisibility()
      .then(() => commands.deviceState())
      .then((state) => {
        if (alive && state) setDeviceState(state);
      })
      .catch(() => {});

    const query =
      typeof window.matchMedia === 'function'
        ? window.matchMedia('(prefers-reduced-motion: reduce)')
        : null;
    const onReduced = () => {
      prefersReduced = query?.matches ?? false;
      notify();
    };
    onReduced();
    query?.addEventListener?.('change', onReduced);

    // Active leaves the focus to the page's visibility (react-query's own reading), so starting
    // up doesn't count as a return and refetch everything.
    focusManager.setEventListener((setFocused) => {
      if (!isAppActive()) setFocused(false);
      // `undefined` must go to `focusManager` itself: its listener's callback ignores it.
      return onAppActiveChange((active) => focusManager.setFocused(active ? undefined : false));
    });

    return () => {
      alive = false;
      void unlisten.then((stop) => stop());
      document.removeEventListener('visibilitychange', onVisibility);
      query?.removeEventListener?.('change', onReduced);
      // Back to react-query's own listener (the page's visibility).
      focusManager.setEventListener((setFocused) => {
        const onChange = () => setFocused();
        window.addEventListener('visibilitychange', onChange, false);
        return () => window.removeEventListener('visibilitychange', onChange);
      });
    };
  }, []);
};
