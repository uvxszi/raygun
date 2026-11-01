import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { changeLanguage } from '.';
import { resolveLanguage } from './languages';

/** Keeps the UI in the language the setting asks for (`system` follows the device). */
export const useSyncLanguage = (setting: string | undefined) => {
  const { i18n } = useTranslation();
  useEffect(() => {
    if (setting === undefined) return;
    const tag = resolveLanguage(setting);
    if (i18n.language !== tag) void changeLanguage(tag);
  }, [i18n, setting]);
};
