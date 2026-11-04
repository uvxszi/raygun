import { usePlatform } from '../lib/usePlatform';

/** Android's line between phones and tablets: a smallest side of 600 dp. */
const TABLET_DP = 600;

/**
 * The screen's smallest side in CSS px (dp on Android), whichever way it's held; 0 when unknown.
 * Interface size (M46.24) scales the root font, never CSS px, so it doesn't move this.
 */
const smallestSide = () =>
  typeof window.screen === 'undefined'
    ? 0
    : Math.min(window.screen.width || 0, window.screen.height || 0);

/**
 * Whether Raygun runs on a phone: Android or iOS with a screen under 600 dp on its short side. A
 * phone keeps its own layouts on its side too, never the desktop's player bar or side panel;
 * tablets and desktop windows go by width as before.
 */
export const useIsPhone = () => {
  const platform = usePlatform();
  if (platform !== 'android' && platform !== 'ios') return false;
  const side = smallestSide();
  return side > 0 && side < TABLET_DP;
};
