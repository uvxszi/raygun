import type { SubtitleStyle } from '@raygun/bindings';
import type { CSSProperties, FC } from 'react';

import { cssColor } from '../settings/playerOptions';

const FONTS: Record<Required<SubtitleStyle>['font'], string> = {
  default: 'inherit',
  sans: 'ui-sans-serif, system-ui, sans-serif',
  serif: 'ui-serif, Georgia, serif',
  mono: 'ui-monospace, Consolas, monospace',
};

const edgeShadow = (edge: Required<SubtitleStyle>['edge'], color: string) => {
  switch (edge) {
    case 'outline':
      return `-2px -2px 0 ${color}, 2px -2px 0 ${color}, -2px 2px 0 ${color}, 2px 2px 0 ${color}, 0 0 3px ${color}`;
    case 'shadow':
      return `3px 3px 4px ${color}`;
    case 'raised':
      return `-1px -1px 0 ${color}`;
    case 'depressed':
      return `1px 1px 0 ${color}`;
    default:
      return 'none';
  }
};

type Props = {
  lines: string[];
  style: Required<SubtitleStyle>;
  /** Moved up while the controls show, so they don't cover the text. */
  lifted: boolean;
  /** The second subtitles show at the top of the picture. */
  at?: 'bottom' | 'top';
};

/** The subtitles over the picture, as Settings → Player → Subtitles styles them. */
export const SubtitleLines: FC<Props> = ({ lines, style, lifted, at = 'bottom' }) => {
  if (lines.length === 0) return null;
  const textStyle: CSSProperties = {
    fontFamily: FONTS[style.font],
    fontWeight: style.bold ? 700 : 500,
    fontStyle: style.italic ? 'italic' : 'normal',
    color: cssColor(style.textColor),
    textShadow: edgeShadow(style.edge, cssColor(style.edgeColor)),
    backgroundColor: cssColor(style.backgroundColor),
    // About 5 % of the picture's height at 100 %.
    fontSize: `calc(clamp(16px, 4.6vh, 56px) * ${style.size / 100})`,
  };
  return (
    <div
      data-testid={at === 'top' ? 'second-subtitle-lines' : 'subtitle-lines'}
      aria-live="off"
      className="pointer-events-none absolute inset-x-0 flex flex-col items-center gap-1 px-[8%] text-center leading-snug transition-[bottom] duration-200"
      style={
        at === 'top'
          ? { top: '6%' }
          : { bottom: lifted ? `max(${style.position}%, 110px)` : `${style.position}%` }
      }
    >
      {lines.map((line, index) => (
        <span
          key={`${index}-${line}`}
          className="rounded px-2 py-0.5 whitespace-pre-wrap"
          style={textStyle}
        >
          {line}
        </span>
      ))}
    </div>
  );
};
