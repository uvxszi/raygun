import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect, useState } from 'react';

/** The app window, or nothing outside Tauri (tests). */
export const appWindow = () => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
};

/** Runs a window call, ignoring platforms and tests without one. */
const withWindow = async (run: (window: NonNullable<ReturnType<typeof appWindow>>) => unknown) => {
  const window = appWindow();
  if (!window) return;
  try {
    await run(window);
  } catch {
    // Android has no window to change.
  }
};

export const minimizeWindow = () => withWindow((window) => window.minimize());
export const toggleMaximizeWindow = () => withWindow((window) => window.toggleMaximize());
export const closeWindow = () => withWindow((window) => window.close());
/** The system's title bar and borders, or Raygun's own bar (M46.9). */
export const setWindowDecorations = (system: boolean) =>
  withWindow((window) => window.setDecorations(system));

/** The whole app's zoom, 1 = 100 % (M46.9). */
export const setAppZoom = async (factor: number) => {
  try {
    await getCurrentWebview().setZoom(factor);
  } catch {
    // Not in Tauri.
  }
};

/** Whether the window is maximized or full screen, kept current as it changes (M46.9). */
export const useWindowState = () => {
  const [state, setState] = useState({ maximized: false, fullscreen: false });
  useEffect(() => {
    const window = appWindow();
    if (!window) return;
    let alive = true;
    let stop: (() => void) | undefined;
    const read = () =>
      void Promise.all([window.isMaximized(), window.isFullscreen()])
        .then(([maximized, fullscreen]) => alive && setState({ maximized, fullscreen }))
        .catch(() => undefined);
    read();
    void window
      .onResized(read)
      .then((unlisten) => {
        if (alive) stop = unlisten;
        else unlisten();
      })
      .catch(() => undefined);
    return () => {
      alive = false;
      stop?.();
    };
  }, []);
  return state;
};
