import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import { FALLBACK_LANGUAGE, loadLanguage } from './languages';
import en from './locales/en.json';

export { FALLBACK_LANGUAGE } from './languages';

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: FALLBACK_LANGUAGE,
  fallbackLng: FALLBACK_LANGUAGE,
  interpolation: { escapeValue: false },
  // Imported translations have keys only where another app had the same English text.
  returnEmptyString: false,
});

/** Switches the UI to `tag`, loading its strings first. */
export const changeLanguage = async (tag: string) => {
  if (tag !== FALLBACK_LANGUAGE && !i18n.hasResourceBundle(tag, 'translation')) {
    const strings = await loadLanguage(tag);
    if (strings) i18n.addResourceBundle(tag, 'translation', strings, true, true);
  }
  await i18n.changeLanguage(tag);
  if (typeof document !== 'undefined') document.documentElement.lang = tag;
};

export default i18n;
