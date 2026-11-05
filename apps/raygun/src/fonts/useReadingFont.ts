import type { NovelFont } from '@raygun/bindings';
import { useTranslation } from 'react-i18next';

import { DEFAULT_PREFS } from '../novel/useNovel';
import { useSettings, useUpdateSetting, useUpdateSettings } from '../settings/useSettings';
import type { FontValue, OwnFont } from './fontValue';
import { FONT_DEFAULTS, READER_STACKS } from './systemFonts';

/**
 * The book reader's font: its own Serif or Sans (`novelReader.font`), or one of the device's
 * fonts in front of it (`fonts.reading`, kept to this device).
 */
export const useReadingFont = () => {
  const { t } = useTranslation();
  const settings = useSettings();
  const update = useUpdateSetting();
  const updateMany = useUpdateSettings();
  const prefs = settings.data?.novelReader ?? DEFAULT_PREFS;
  const fonts = settings.data?.fonts ?? FONT_DEFAULTS;
  const own: OwnFont[] = [
    { id: 'serif', label: t('settings.fontSerif'), stack: READER_STACKS.serif },
    { id: 'sans', label: t('settings.fontSans'), stack: READER_STACKS.sans },
  ];
  const value: FontValue = fonts.reading
    ? { kind: 'family', family: fonts.reading }
    : { kind: 'own', id: prefs.font };
  const pick = (next: FontValue) => {
    if (next.kind === 'family') {
      update.mutate({ key: 'fonts', value: { ...fonts, reading: next.family } });
      return;
    }
    const typeface = {
      key: 'novelReader',
      value: { ...prefs, font: next.id as NovelFont },
    } as const;
    // Serif or Sans again puts the device's font away, both saved together.
    if (fonts.reading) {
      updateMany.mutate([typeface, { key: 'fonts', value: { ...fonts, reading: '' } }]);
    } else {
      update.mutate(typeface);
    }
  };
  return { own, value, pick, fallback: READER_STACKS[prefs.font] };
};
