import type { SubtitleEdge, SubtitleFont, SubtitleStyle } from '@raygun/bindings';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { cssColor } from './playerOptions';

const FAMILIES: Record<SubtitleFont, string> = {
  default: 'inherit',
  sans: 'sans-serif',
  serif: 'serif',
  mono: 'monospace',
};

const edgeShadow = (edge: SubtitleEdge, color: string) => {
  switch (edge) {
    case 'outline':
      return `-1px -1px 0 ${color}, 1px -1px 0 ${color}, -1px 1px 0 ${color}, 1px 1px 0 ${color}`;
    case 'shadow':
      return `2px 2px 3px ${color}`;
    case 'raised':
      return `-1px -1px 0 ${color}`;
    case 'depressed':
      return `1px 1px 0 ${color}`;
    default:
      return 'none';
  }
};

/** A line of subtitles over a dark picture, styled as the player will draw it. */
export const SubtitlePreview: FC<{ style: Required<SubtitleStyle> }> = ({ style }) => {
  const { t } = useTranslation();
  return (
    <div
      data-testid="subtitle-preview"
      className="relative h-40 overflow-hidden rounded-xl bg-gradient-to-br from-slate-500 via-slate-700 to-slate-950"
    >
      <p className="absolute inset-x-0 px-4 text-center" style={{ bottom: `${style.position}%` }}>
        <span
          data-testid="subtitle-preview-text"
          style={{
            color: cssColor(style.textColor),
            backgroundColor: cssColor(style.backgroundColor),
            textShadow: edgeShadow(style.edge, cssColor(style.edgeColor)),
            fontFamily: FAMILIES[style.font],
            fontWeight: style.bold ? 700 : 400,
            fontStyle: style.italic ? 'italic' : 'normal',
            fontSize: `${(style.size / 100) * 1.125}rem`,
            padding: '0.1em 0.3em',
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
