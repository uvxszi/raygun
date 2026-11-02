import { useEffect, useState } from 'react';

import { appWindow } from './appWindow';

/** Turns the window's full screen on or off (F11, the readers' button; M46.7). */
export const toggleWindowFullscreen = async () => {
  const window = appWindow();
  if (!window) return false;
  try {
    const next = !(await window.isFullscreen());
    await window.setFullscreen(next);
    return next;
  } catch {
    return false;
  }
};

/** Whether the window is full screen, and a way to switch; `available` is false off desktop. */
export const useWindowFullscreen = () => {
  const [on, setOn] = useState(false);
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    const window = appWindow();
    let alive = true;
    void window
      ?.isFullscreen()
      .then((state) => {
        if (!alive) return;
        setOn(state);
        setAvailable(true);
      })
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, []);
  return {
    on,
    available,
    toggle: async () => setOn(await toggleWindowFullscreen()),
  };
};
