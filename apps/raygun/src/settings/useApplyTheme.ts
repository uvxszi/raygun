import type { Theme } from '@raygun/bindings';
import { useEffect } from 'react';

export const useApplyTheme = (theme: Theme | undefined) => {
  useEffect(() => {
    const root = document.documentElement;
    if (!theme || theme === 'system') {
      delete root.dataset.theme;
    } else {
      root.dataset.theme = theme;
    }
  }, [theme]);
};
