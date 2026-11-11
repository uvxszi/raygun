import type { SubtitleStyle } from '@raygun/bindings';
import type { CSSProperties, FC } from 'react';

import { cssColor } from '../settings/playerOptions';
import { subtitleFontPx } from './pictureBox';
import type { PictureBox } from './pictureBox';
import { subtitlePlacement, subtitleSlide } from './subtitlePlacement';
import type { SubtitlePlace } from './subtitlePlacement';

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
  /**
   * The controls show: the lines slide out of their way, keeping their size (up above the
   * bottom bar, or for the second subtitles down below the top bar).
   */
  lifted: boolean;
  /** The second subtitles show at the top of the picture. */
  at?: SubtitlePlace;
  /** Where the picture shows; null before the player has a size. */
  box?: PictureBox | null;
};

/**
 * The subtitles over the picture, as Settings → Player → Subtitles styles them. Each line wraps
 * within the chosen width, and a background shows behind each line it wraps to in a box with
 * rounded corners. Showing and hiding the controls slides the lines (a transform) rather than
 * placing them again, so they keep their size and wrapping.
 */
export const SubtitleLines: FC<Props> = ({ lines, style, lifted, at = 'bottom', box }) => {
  if (lines.length === 0) return null;
  const textStyle: CSSProperties = {
    fontFamily: FONTS[style.font],
    fontWeight: style.bold ? 700 : 500,
    fontStyle: style.italic ? 'italic' : 'normal',
    color: cssColor(style.textColor),
    textShadow: edgeShadow(style.edge, cssColor(style.edgeColor)),
    backgroundColor: cssColor(style.backgroundColor),
    fontSize:
      box && box.height > 0
        ? `${subtitleFontPx(box, style.size)}px`
        : `calc(clamp(16px, 4.6vh, 56px) * ${style.size / 100})`,
    padding: '0.06em 0.35em',
    borderRadius: '0.25em',
    boxDecorationBreak: 'clone',
    WebkitBoxDecorationBreak: 'clone',
  };
  const slide = subtitleSlide(box, style, lifted, at);
  return (
    <div
      data-testid={at === 'top' ? 'second-subtitle-lines' : 'subtitle-lines'}
      data-slide={slide}
      aria-live="off"
      className="pointer-events-none absolute flex flex-col items-center gap-1 text-center leading-snug transition-transform duration-200 ease-out"
      style={{
        ...subtitlePlacement(box, style, lifted, at),
        transform: slide === 0 ? undefined : `translateY(${slide}px)`,
      }}
    >
      {lines.map((line, index) => (
        <p key={`${index}-${line}`} className="max-w-full">
          <span className="whitespace-pre-wrap" style={textStyle}>
            {line}
          </span>
        </p>
      ))}
    </div>
  );
};
