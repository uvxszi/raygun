import type { SubtitleStyle } from '@raygun/bindings';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import {
  BACKGROUND_COLORS,
  EDGE_COLORS,
  SUBTITLE_EDGES,
  SUBTITLE_MAX_WIDTH,
  TEXT_COLORS,
} from './playerOptions';
import { PrefChoice, PrefSize, PrefSlider, PrefSwitch } from './prefs';
import { PrefColor } from './PrefColor';
import { SubtitlePreview } from './SubtitlePreview';

const colorOptions = (colors: { value: number; name: string }[], t: (key: string) => string) =>
  colors.map((color) => ({ value: color.value, label: t(`settings.player.colors.${color.name}`) }));

type Props = {
  style: Required<SubtitleStyle>;
  setStyle: (change: Partial<SubtitleStyle>) => void;
  onReset: () => void;
  /** The reset row's words ("Reset the style", "Like the main subtitles"). */
  resetLabel: string;
  /** The font row: Settings' own picker, or the second subtitles' families. */
  font: ReactNode;
  /** A device font drawn in front of the style's own family. */
  family?: string | null;
  busy?: boolean;
  /** The phone player's own row (ignoring a file's styles). */
  phone?: boolean;
  /** The height's words: from the picture's foot (main), or its top (second subtitles). */
  positionTitle?: string;
};

/**
 * A subtitle style's rows: a sample, size, font, bold, italic, colours, edge, height and line
 * width, then a reset. Settings → Video player → Subtitles has them for the main subtitles, the
 * desktop player's second subtitles for theirs (kept per title).
 */
export const SubtitleStyleRows: FC<Props> = ({
  style,
  setStyle,
  onReset,
  resetLabel,
  font,
  family = null,
  busy = false,
  phone = false,
  positionTitle,
}) => {
  const { t } = useTranslation();
  return (
    <>
      <li className="px-1 py-2">
        <SubtitlePreview style={style} family={family} />
      </li>
      <PrefSize
        title={t('settings.player.subtitleSize')}
        unit="%"
        zero={100}
        min={50}
        max={200}
        step={10}
        value={style.size}
        disabled={busy}
        onChange={(size) => setStyle({ size })}
      />
      {font}
      <PrefSwitch
        title={t('settings.player.bold')}
        checked={style.bold}
        disabled={busy}
        onChange={(bold) => setStyle({ bold })}
      />
      <PrefSwitch
        title={t('settings.player.italic')}
        checked={style.italic}
        disabled={busy}
        onChange={(italic) => setStyle({ italic })}
      />
      {/* A colour picked wins over the file's own colours, white and "None" too. */}
      <PrefColor
        title={t('settings.player.textColor')}
        value={style.textColor}
        disabled={busy}
        colors={colorOptions(TEXT_COLORS, t)}
        onChange={(textColor) => setStyle({ textColor, textColorChosen: true })}
      />
      <PrefChoice
        title={t('settings.player.edge')}
        value={style.edge}
        disabled={busy}
        options={SUBTITLE_EDGES.map((value) => ({
          value,
          label: t(`settings.player.edges.${value}`),
        }))}
        onChange={(edge) => setStyle({ edge })}
      />
      <PrefColor
        title={t('settings.player.edgeColor')}
        value={style.edgeColor}
        disabled={busy || style.edge === 'none'}
        colors={colorOptions(EDGE_COLORS, t)}
        onChange={(edgeColor) => setStyle({ edgeColor })}
      />
      <PrefColor
        title={t('settings.player.backgroundColor')}
        value={style.backgroundColor}
        disabled={busy}
        colors={colorOptions(BACKGROUND_COLORS, t)}
        opacity
        onChange={(backgroundColor) => setStyle({ backgroundColor, backgroundColorChosen: true })}
      />
      <PrefSlider
        title={positionTitle ?? t('settings.player.subtitlePosition')}
        value={style.position}
        min={0}
        max={40}
        step={2}
        format={(value) => `${value}%`}
        disabled={busy}
        onChange={(position) => setStyle({ position })}
      />
      <PrefSlider
        title={t('settings.player.subtitleMaxWidth')}
        summary={t('settings.player.subtitleMaxWidthHint')}
        value={style.maxWidth}
        min={SUBTITLE_MAX_WIDTH.min}
        max={SUBTITLE_MAX_WIDTH.max}
        step={SUBTITLE_MAX_WIDTH.step}
        format={(value) => `${value}%`}
        disabled={busy}
        onChange={(maxWidth) => setStyle({ maxWidth })}
      />
      {phone && (
        <PrefSwitch
          title={t('settings.player.ignoreEmbedded')}
          summary={t('settings.player.ignoreEmbeddedHint')}
          checked={style.ignoreEmbedded}
          disabled={busy}
          onChange={(ignoreEmbedded) => setStyle({ ignoreEmbedded })}
        />
      )}
      <li>
        <button
          type="button"
          disabled={busy}
          onClick={onReset}
          className="text-primary hover:bg-surface w-full rounded-xl px-1 py-3 text-left font-medium disabled:opacity-60"
        >
          {resetLabel}
        </button>
      </li>
    </>
  );
};
