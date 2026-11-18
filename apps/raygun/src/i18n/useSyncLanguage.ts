import { commands } from '@raygun/bindings';
import type { TFunction } from 'i18next';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

import { changeLanguage } from '.';
import { resolveLanguage } from './languages';
import en from './locales/en.json';

/** Every key under `tree`, dotted, as the locale files spell them. */
const keysOf = (tree: Record<string, unknown>, prefix = ''): string[] =>
  Object.entries(tree).flatMap(([key, value]) =>
    value !== null && typeof value === 'object'
      ? keysOf(value as Record<string, unknown>, `${prefix}${key}.`)
      : [`${prefix}${key}`],
  );

const NOTICE_KEYS = keysOf(en.notices);

/**
 * The words Rust shows by itself (notifications), in the UI's language: `notices.*` resolved
 * for it, keyed without the prefix (`raygun_app_base::texts`).
 */
export const noticeTexts = (translate: TFunction): Record<string, string> =>
  Object.fromEntries(NOTICE_KEYS.map((key) => [key, translate(`notices.${key}`)]));

/**
 * Keeps the UI in the language the setting asks for (`system` follows the device), and Rust's
 * notifications with it.
 */
export const useSyncLanguage = (setting: string | undefined) => {
  const { i18n } = useTranslation();
  useEffect(() => {
    if (setting === undefined) return;
    const tag = resolveLanguage(setting);
    const switched = i18n.language === tag ? Promise.resolve() : changeLanguage(tag);
    let current = true;
    void switched
      .then(async () => {
        if (current) await commands.appTextsSet(tag, noticeTexts(i18n.getFixedT(tag)));
      })
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [i18n, setting]);
};
