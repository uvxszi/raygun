import { createContext, useContext } from 'react';

/**
 * How long a finger holds an item before it's picked, ms: shorter than Android's own long press
 * (400 ms), so the webview never gets to start dragging the link or picture under the finger,
 * which would take the touch away before the pick.
 */
export const HOLD_MS = 350;

/** Set inside a `SelectionScope`: results there can be picked. */
export const ScopeContext = createContext(false);

/** Whether the results here can be picked: inside a `SelectionScope`. */
export const useInSelectionScope = () => useContext(ScopeContext);
