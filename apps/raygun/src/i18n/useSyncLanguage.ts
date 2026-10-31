import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

export const useSyncLanguage = (language: string | undefined) => {
  const { i18n } = useTranslation();
  useEffect(() => {
    if (language && i18n.language !== language) {
      void i18n.changeLanguage(language);
    }
  }, [i18n, language]);
};
