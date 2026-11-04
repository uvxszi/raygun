import { createContext, useContext } from 'react';

/** How long a finger holds a result before it's picked, ms. */
export const HOLD_MS = 450;

/** Set inside a `SelectionScope`: results there can be picked. */
export const ScopeContext = createContext(false);

/** Whether the results here can be picked: inside a `SelectionScope`. */
export const useInSelectionScope = () => useContext(ScopeContext);
