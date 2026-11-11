/**
 * The screen's safe area: how far (CSS px) the status bar, the navigation bar or gesture area and
 * a camera cutout reach in from each edge. Styles use `--safe-top`, `--safe-right`,
 * `--safe-bottom` and `--safe-left` (styles.css: the webview's own insets, or on Android what the
 * activity measures and sets on the root, MainActivity's `SafeArea`); this reads them for placing
 * things in script.
 */
export type SafeInsets = { top: number; right: number; bottom: number; left: number };

const SIDES = ['top', 'right', 'bottom', 'left'] as const;

let probe: HTMLElement | null = null;

/** An element padded by the safe area, for the webview to resolve its insets on. */
const probeElement = () => {
  if (probe?.isConnected) return probe;
  probe = document.createElement('div');
  probe.setAttribute('aria-hidden', 'true');
  probe.style.cssText =
    'position:fixed;top:0;left:0;width:0;height:0;visibility:hidden;pointer-events:none;' +
    'padding:var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left)';
  document.body.append(probe);
  return probe;
};

const px = (value: string) => {
  const number = Number.parseFloat(value);
  return Number.isFinite(number) && number > 0 ? number : 0;
};

/** The safe area now. */
export const safeInsets = (): SafeInsets => {
  if (typeof document === 'undefined' || !document.body) {
    return { top: 0, right: 0, bottom: 0, left: 0 };
  }
  // Set as lengths (Android's activity sets them so): read as they are.
  const root = getComputedStyle(document.documentElement);
  const set = SIDES.map((side) => root.getPropertyValue(`--safe-${side}`).trim());
  if (set.every((value) => /^-?\d+(\.\d+)?px$/.test(value))) {
    const [top = 0, right = 0, bottom = 0, left = 0] = set.map(px);
    return { top, right, bottom, left };
  }
  const padded = getComputedStyle(probeElement());
  return {
    top: px(padded.paddingTop),
    right: px(padded.paddingRight),
    bottom: px(padded.paddingBottom),
    left: px(padded.paddingLeft),
  };
};
