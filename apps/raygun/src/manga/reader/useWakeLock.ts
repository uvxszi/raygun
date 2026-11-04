import { useEffect } from 'react';

/**
 * Keeps the screen on while `enabled` (a reader is open): the Screen Wake Lock API, asked for
 * again when the app comes back, since the system drops it while the app is hidden.
 */
export const useWakeLock = (enabled: boolean) => {
  useEffect(() => {
    if (!enabled || typeof navigator === 'undefined' || !('wakeLock' in navigator)) return;
    let lock: WakeLockSentinel | null = null;
    let done = false;
    const request = () => {
      if (document.visibilityState !== 'visible' || (lock && !lock.released)) return;
      navigator.wakeLock
        .request('screen')
        .then((sentinel) => {
          if (done) void sentinel.release().catch(() => undefined);
          else lock = sentinel;
        })
        // Refused (battery saver, an old webview): the screen just times out as usual.
        .catch(() => undefined);
    };
    request();
    document.addEventListener('visibilitychange', request);
    return () => {
      done = true;
      document.removeEventListener('visibilitychange', request);
      void lock?.release().catch(() => undefined);
    };
  }, [enabled]);
};
