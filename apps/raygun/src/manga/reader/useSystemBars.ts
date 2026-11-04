import { commands } from '@raygun/bindings';
import { useEffect } from 'react';

import { usePlatform } from '../../lib/usePlatform';

/**
 * A reader's full screen on Android: the status and navigation bars are hidden while `hidden`
 * (a swipe from an edge shows them for a moment), and come back when it turns false or the reader
 * closes. Desktop has F11 instead.
 */
export const useSystemBars = (hidden: boolean) => {
  const android = usePlatform() === 'android';
  useEffect(() => {
    if (!android || !hidden) return;
    void commands.readerSystemBars(true).catch(() => undefined);
    return () => {
      void commands.readerSystemBars(false).catch(() => undefined);
    };
  }, [android, hidden]);
};
