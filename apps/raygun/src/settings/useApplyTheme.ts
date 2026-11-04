import type { Accent, CustomColors, MotionLevel, ScrollbarStyle, Theme } from '@raygun/bindings';
import { useEffect } from 'react';

import { useAppActive, useEffectiveMotion } from '../shell/deviceState';

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

/** The user's own colours over the theme's (M46.8): set on the root, they win over any theme. */
export const useApplyColors = (colors: Partial<CustomColors> | undefined) => {
  const { background = '', surface = '', foreground = '', muted = '' } = colors ?? {};
  useEffect(() => {
    const root = document.documentElement;
    for (const [key, value] of Object.entries({ background, surface, foreground, muted })) {
      if (HEX.test(value)) root.style.setProperty(`--color-${key}`, value);
      else root.style.removeProperty(`--color-${key}`);
    }
  }, [background, surface, foreground, muted]);
};

/**
 * Puts the scrollbar style, the animation level in effect and whether Raygun is on screen on the
 * page (M46.7; styles.css): `data-motion` is the setting lowered by the system's animation
 * settings, battery saver and reduced motion (`lively`, `calm`, `reduced` or `off`), and
 * `data-active="false"` pauses every CSS animation while Raygun is in the background.
 */
export const useApplyFeel = (
  scrollbars: ScrollbarStyle | undefined = 'thin',
  motion: MotionLevel | undefined = 'lively',
) => {
  const effective = useEffectiveMotion(motion);
  const active = useAppActive();
  useEffect(() => {
    document.documentElement.dataset.scrollbars = scrollbars;
  }, [scrollbars]);
  useEffect(() => {
    document.documentElement.dataset.motion = effective;
  }, [effective]);
  useEffect(() => {
    document.documentElement.dataset.active = String(active);
  }, [active]);
  return effective;
};
