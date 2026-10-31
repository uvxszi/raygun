// Entry of the converter bundle Raygun runs in QuickJS at install time:
// `__convert(filesJson)` → `{ manifest, bundle, report }` or `{ error }` as JSON.
import shim from 'virtual:shim';

import { convertPlugin } from './convert';

(globalThis as { __convert?: (json: string) => string }).__convert = (json: string) => {
  try {
    return JSON.stringify(convertPlugin(JSON.parse(json) as Record<string, string>, shim));
  } catch (error) {
    return JSON.stringify({ error: error instanceof Error ? error.message : String(error) });
  }
};
