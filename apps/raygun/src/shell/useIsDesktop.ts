import { usePlatform } from '../lib/usePlatform';

/** Whether Raygun runs as a desktop app (Windows, Linux, macOS), once known (M46.9). */
export const useIsDesktop = () => {
  const platform = usePlatform();
  return platform !== undefined && platform !== 'android' && platform !== 'ios';
};
