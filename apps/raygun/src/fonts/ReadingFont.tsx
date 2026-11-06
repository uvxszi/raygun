import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { FontPickerSheet, PrefFont } from './FontPicker';
import { useFontLabel } from './fontValue';
import type { OwnFont } from './fontValue';
import { useReadingFont } from './useReadingFont';

/** Settings → Reader's Typeface row: Serif, Sans or a font of the device's. */
export const ReadingFontRow: FC = () => {
  const { t } = useTranslation();
  const font = useReadingFont();
  return (
    <PrefFont
      title={t('settings.font')}
      own={font.own}
      value={font.value}
      fallback={font.fallback}
      onPick={font.pick}
    />
  );
};

/**
 * The reader's own panel: Serif, Sans, and the device's fonts in a sheet. `buttonClass` is the
 * panel's button style, given whether the button is the current choice.
 */
export const ReadingFontChoices: FC<{ buttonClass: (pressed: boolean) => string }> = ({
  buttonClass,
}) => {
  const { t } = useTranslation();
  const font = useReadingFont();
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const device = font.value.kind === 'family';
  const current = useFontLabel(font.own, font.value, font.fallback);
  const ownChoice = (option: OwnFont | undefined, label: string) => {
    if (!option) return null;
    const pressed = font.value.kind === 'own' && font.value.id === option.id;
    return (
      <button
        type="button"
        aria-pressed={pressed}
        onClick={() => font.pick({ kind: 'own', id: option.id })}
        className={buttonClass(pressed)}
        style={{ fontFamily: option.stack }}
      >
        {label}
      </button>
    );
  };
  const [serif, sans] = font.own;
  return (
    <>
      <div className="flex gap-2">
        {ownChoice(serif, t('novel.serif'))}
        {ownChoice(sans, t('novel.sans'))}
      </div>
      <button
        type="button"
        aria-pressed={device}
        aria-haspopup="dialog"
        onClick={(event) => setAnchor(anchor ? null : event.currentTarget)}
        className={`${buttonClass(device)} truncate`}
        style={device ? { fontFamily: current.stack } : undefined}
      >
        {device ? current.label : t('settings.fonts.more')}
      </button>
      {anchor && (
        <FontPickerSheet
          title={t('settings.font')}
          own={font.own}
          value={font.value}
          fallback={font.fallback}
          onPick={font.pick}
          anchor={anchor}
          onClose={() => setAnchor(null)}
        />
      )}
    </>
  );
};
