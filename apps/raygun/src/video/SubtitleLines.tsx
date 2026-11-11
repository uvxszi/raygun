import type { SubtitleStyle } from '@raygun/bindings';
import type { CSSProperties, FC } from 'react';

import { cssColor } from '../settings/playerOptions';
import { subtitleFontPx } from './pictureBox';
import type { PictureBox } from './pictureBox';

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
  /** Where the picture shows; null before the player has a size. */
  box?: PictureBox | null;
};

/** How far up the control bar reaches over the player's foot; lifted lines stay above it. */
const BAR_PX = 110;

/** Where the lines go: over the picture's own part of the player, at the chosen height in it. */
const placement = (
  box: PictureBox | null | undefined,
  style: Required<SubtitleStyle>,
  lifted: boolean,
  at: 'bottom' | 'top',
): CSSProperties => {
  if (!box || box.height <= 0) {
    return at === 'top'
      ? { top: '6%', left: 0, right: 0 }
      : {
          bottom: lifted ? `max(${style.position}%, ${BAR_PX}px)` : `${style.position}%`,
          left: 0,
          right: 0,
        };
  }
  const sides = { left: box.left, width: box.width };
  if (at === 'top') return { ...sides, top: box.top + box.height * 0.06 };
  const foot = box.stageHeight - (box.top + box.height);
  const chosen = foot + (box.height * style.position) / 100;
  return { ...sides, bottom: lifted ? Math.max(chosen, BAR_PX) : chosen };
};

/** The subtitles over the picture, as Settings → Player → Subtitles styles them. */
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
  };
  return (
    <div
      data-testid={at === 'top' ? 'second-subtitle-lines' : 'subtitle-lines'}
      aria-live="off"
      className="pointer-events-none absolute flex flex-col items-center gap-1 px-[8%] text-center leading-snug transition-[bottom] duration-200"
      style={placement(box, style, lifted, at)}
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
