import type { SubtitleFont } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PrefFont } from '../fonts/FontPicker';
import type { FontValue, OwnFont } from '../fonts/fontValue';
import { FONT_DEFAULTS, INTERFACE_STACK } from '../fonts/systemFonts';
import { usePlatform } from '../lib/usePlatform';
import { subtitleFontCss } from '../video/subtitleLook';
import { SUBTITLE_FONTS, withDefaults } from './playerOptions';
import { useSettings, useUpdateSetting, useUpdateSettings } from './useSettings';

/** A subtitle font as the picker lists it: named in its own font. */
const ownFont = (id: SubtitleFont, label: string): OwnFont => {
  const { fontFamily, ...css } = subtitleFontCss(id);
  return { id, label, stack: fontFamily ?? 'inherit', css };
};

/**
 * Settings → Video player → Subtitles' Font: the families every player draws (Android's caption
 * families), each named in its own font. On desktop the device's fonts follow, kept to the device
 * (`fonts.subtitles`) and drawn in front of the chosen family; Android's player draws the system's
 * families only, so its list stops there.
 */
export const SubtitleFontRow: FC<{ title: string }> = ({ title }) => {
  const { t } = useTranslation();
  const platform = usePlatform();
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const updateMany = useUpdateSettings();
  if (!settings) return null;
  const prefs = withDefaults(settings.videoPlayer);
  const style = prefs.subtitleStyle;
  const fonts = settings.fonts ?? FONT_DEFAULTS;
  const deviceFonts = platform !== undefined && platform !== 'android';
  const own = SUBTITLE_FONTS.map((id) => ownFont(id, t(`settings.player.fonts.${id}`)));
  const value: FontValue =
    deviceFonts && fonts.subtitles
      ? { kind: 'family', family: fonts.subtitles }
      : { kind: 'own', id: style.font };
  const current = own.find((option) => option.id === style.font)?.stack ?? 'inherit';
  const pick = (next: FontValue) => {
    if (next.kind === 'family') {
      update.mutate({ key: 'fonts', value: { ...fonts, subtitles: next.family } });
      return;
    }
    const typeface = {
      key: 'videoPlayer',
      value: { ...prefs, subtitleStyle: { ...style, font: next.id as SubtitleFont } },
    } as const;
    // One of the families again puts the device's font away, both saved together.
    if (fonts.subtitles) {
      updateMany.mutate([typeface, { key: 'fonts', value: { ...fonts, subtitles: '' } }]);
    } else {
      update.mutate(typeface);
    }
  };
  return (
    <PrefFont
      title={title}
      own={own}
      value={value}
      fallback={current === 'inherit' ? INTERFACE_STACK : current}
      onPick={pick}
      deviceFonts={deviceFonts}
    />
  );
};
