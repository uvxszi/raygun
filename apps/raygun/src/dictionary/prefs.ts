import type { DictionaryPrefs } from '@raygun/bindings';

import { useSettings } from '../settings/useSettings';

/** The reader's dictionary as Rust sets it up by default: on, Raygun's own sources in order. */
export const DICTIONARY_DEFAULTS: Required<DictionaryPrefs> = {
  enabled: true,
  order: [],
  off: [],
  language: '',
};

/** The reader's dictionary settings (Settings → Reader → Dictionary), every field given. */
export const useDictionaryPrefs = (): Required<DictionaryPrefs> => {
  const prefs = useSettings().data?.dictionary;
  return { ...DICTIONARY_DEFAULTS, ...prefs } as Required<DictionaryPrefs>;
};
