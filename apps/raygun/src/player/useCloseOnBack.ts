import { useEffect } from 'react';

const SHEET_STATE = 'raygunSheet';

const isSheetEntry = (state: unknown) =>
  typeof state === 'object' && state !== null && SHEET_STATE in state;

/**
 * Makes the system Back button (Android) close an overlay instead of leaving the page: while open,
 * an extra history entry with the same URL sits on top, and going back from it closes the overlay.
 * Closing the overlay any other way removes that entry again.
 */
export const useCloseOnBack = (open: boolean, close: () => void) => {
  useEffect(() => {
    if (!open) {
      return;
    }
    window.history.pushState({ [SHEET_STATE]: true }, '');
    const onPopState = (event: PopStateEvent) => {
      // Landing on another overlay entry (e.g. a re-open racing a close) isn't a Back press.
      if (!isSheetEntry(event.state)) {
        close();
      }
    };
    window.addEventListener('popstate', onPopState);
    return () => {
      window.removeEventListener('popstate', onPopState);
      if (isSheetEntry(window.history.state)) {
        window.history.back();
      }
    };
  }, [open, close]);
};
