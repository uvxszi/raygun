import type { SubtitleStyle } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { edgeShadow, subtitleFontCss } from '../video/subtitleLook';
import { cssColor } from './playerOptions';

/**
 * A line of subtitles over a dark picture, styled as the player will draw it; `family` is a font
 * of the device's picked for subtitles.
 */
export const SubtitlePreview: FC<{ style: Required<SubtitleStyle>; family?: string | null }> = ({
  style,
  family,
}) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="subtitle-preview"
      className="relative h-40 overflow-hidden rounded-xl bg-gradient-to-br from-slate-500 via-slate-700 to-slate-950"
    >
      <p
        className="absolute inset-x-0 text-center"
        style={{
          bottom: `${style.position}%`,
          // Lines wrap within the chosen share of the width, as over a video.
          paddingLeft: `${(100 - style.maxWidth) / 2}%`,
          paddingRight: `${(100 - style.maxWidth) / 2}%`,
        }}
      >
        <span
          data-testid="subtitle-preview-text"
          style={{
            color: cssColor(style.textColor),
            backgroundColor: cssColor(style.backgroundColor),
            textShadow: edgeShadow(style.edge, cssColor(style.edgeColor), 1),
            ...subtitleFontCss(style.font, family),
            fontWeight: style.bold ? 700 : 400,
            fontStyle: style.italic ? 'italic' : 'normal',
            fontSize: `${(style.size / 100) * 1.125}rem`,
            padding: '0.1em 0.3em',
            borderRadius: '0.25em',
            boxDecorationBreak: 'clone',
            WebkitBoxDecorationBreak: 'clone',
          }}
        >
          {t('settings.player.subtitlePreview')}
        </span>
      </p>
    </div>
  );
};
