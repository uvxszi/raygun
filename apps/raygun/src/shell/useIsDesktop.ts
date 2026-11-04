import { usePlatform } from '../lib/usePlatform';

/** Whether Raygun runs as a desktop app (Windows, Linux, macOS), once known (M46.9). */
export const useIsDesktop = () => {
  const platform = usePlatform();
  return platform !== undefined && platform !== 'android' && platform !== 'ios';
};

/** Whether Raygun runs on a phone or tablet (Android, iOS): everything but a known desktop. */
export const useIsPhone = () => !useIsDesktop();

/** The one platform a control, setting or menu item belongs to; unset means both. */
export type PlatformOnly = 'desktop' | 'phone';

/** Whether something marked `only` shows on this platform (`desktop` from `useIsDesktop`). */
export const shownOn = (only: PlatformOnly | undefined, desktop: boolean) =>
  only === undefined || (only === 'desktop') === desktop;
