import type { Accent, Theme } from '@raygun/bindings';
import { useEffect } from 'react';

/** Puts the theme and accent on the page; "system" follows the device's light or dark mode. */
export const useApplyTheme = (theme: Theme | undefined, accent: Accent | undefined = 'red') => {
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
    if (!accent || accent === 'red') {
      delete root.dataset.accent;
    } else {
      root.dataset.accent = accent;
    }
  }, [accent]);
};
