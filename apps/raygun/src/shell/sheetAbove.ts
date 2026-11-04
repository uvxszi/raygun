import { createContext } from 'react';

/**
 * Sheets opened from inside something drawn above the app (the video player's settings panel)
 * go above it too.
 */
export const SheetAboveContext = createContext(false);
