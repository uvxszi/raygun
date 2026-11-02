import type { Accent, MotionLevel, ScrollbarStyle, Theme } from '@raygun/bindings';
import { useEffect } from 'react';

const HEX = /^#[0-9a-f]{6}$/i;

/**
 * A chosen colour as the accent (M46.7): its hue and chroma, with the lightness kept where text on
 * it and it on the background stay readable in each theme.
 */
export const customAccent = (color: string) => ({
  onDark: `oklch(from ${color} clamp(0.62, l, 0.88) c h)`,
  onLight: `oklch(from ${color} clamp(0.4, l, 0.6) c h)`,
});

/** Puts the theme and accent on the page; "system" follows the device's light or dark mode. */
export const useApplyTheme = (
  theme: Theme | undefined,
  accent: Accent | undefined = 'red',
  accentColor?: string,
) => {
  useEffect(() => {
    const root = document.documentElement;
    if (!theme || theme === 'system') {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = theme;
    }
  }, [theme]);
  useEffect(() => {
    const root = document.documentElement;
    const custom = accent === 'custom' && accentColor && HEX.test(accentColor);
    if (custom) {
      const { onDark, onLight } = customAccent(accentColor);
      root.dataset.accent = 'custom';
      root.style.setProperty('--accent-on-dark', onDark);
      root.style.setProperty('--accent-on-light', onLight);
      return;
    }
    root.style.removeProperty('--accent-on-dark');
    root.style.removeProperty('--accent-on-light');
    if (!accent || accent === 'red' || accent === 'custom') {
      delete root.dataset.accent;
    } else {
      root.dataset.accent = accent;
    }
  }, [accent, accentColor]);
};

/** Puts the scrollbar style and the animation level on the page (M46.7; styles.css). */
export const useApplyFeel = (
  scrollbars: ScrollbarStyle | undefined = 'thin',
  motion: MotionLevel | undefined = 'lively',
) => {
  useEffect(() => {
    document.documentElement.dataset.scrollbars = scrollbars;
  }, [scrollbars]);
  useEffect(() => {
    document.documentElement.dataset.motion = motion;
  }, [motion]);
};
