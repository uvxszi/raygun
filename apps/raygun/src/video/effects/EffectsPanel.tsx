import { RotateCcw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PanelHeading, PanelOption, PlayerPanel } from '../PlayerPanel';
import { ADJUSTMENT_LABEL, HEAVY_LABEL, LOOK_LABEL, SMART_AUDIO } from './effectLabels';
import {
  adjustmentsOf,
  ADJUSTMENTS,
  EFFECT_PRESETS,
  LOOKS,
  sameEffects,
  shownValue,
} from './effectParams';
import type { Adjustment, HeavyKind } from './effectParams';
import type { VideoEffects } from './useVideoEffects';

const pill = (pressed: boolean) =>
  `rounded-full px-3 py-1.5 text-sm ${pressed ? 'bg-white text-black' : 'bg-white/10 hover:bg-white/20'}`;

type SliderProps = {
  name: string;
  adjustment: Adjustment;
  value: number;
  onChange: (value: number) => void;
};

/** One adjustment: its name and value over a slider, and a button back to 0. */
const EffectSlider: FC<SliderProps> = ({ name, adjustment, value, onChange }) => {
  const { t } = useTranslation();
  return (
    <div className="flex items-end gap-1 px-3 py-1">
      <div className="flex min-w-0 flex-1 flex-col gap-1 text-sm">
        <div className="flex justify-between gap-2">
          <span className="truncate">{name}</span>
          <span className="text-white/70 tabular-nums">{shownValue(adjustment.key, value)}</span>
        </div>
        <input
          type="range"
          aria-label={name}
          min={adjustment.min}
          max={adjustment.max}
          step={1}
          value={value}
          onChange={(event) => onChange(Number(event.target.value))}
          className="accent-primary w-full"
        />
      </div>
      <button
        type="button"
        aria-label={t('settings.sizeReset', { name })}
        disabled={value === 0}
        onClick={() => onChange(0)}
        className="grid size-8 place-items-center rounded-full hover:bg-white/10 disabled:opacity-30"
      >
        <RotateCcw aria-hidden size={14} />
      </button>
    </div>
  );
};

type Props = { effects: VideoEffects; onClose: () => void };

/**
 * More → Effects in the desktop player: presets, a look, and a slider per adjustment, changing
 * the picture as they move (kept for the title once they settle), with Reset all, Make default
 * and, for a title with its own, Use the default.
 */
export const EffectsPanel: FC<Props> = ({ effects, onClose }) => {
  const { t } = useTranslation();
  const { picture } = effects;
  const names = (kinds: HeavyKind[]) => kinds.map((kind) => t(HEAVY_LABEL[kind])).join(', ');
  const note = effects.unavailable
    ? t('videoPlayer.effectsNoWebGl')
    : effects.dropped.length
      ? t('videoPlayer.effectsDropped', { n: names(effects.dropped) })
      : null;
  return (
    <PlayerPanel title={t('videoPlayer.effects')} onClose={onClose}>
      <PanelHeading>{t('videoPlayer.effectsPresets')}</PanelHeading>
      <div className="flex flex-wrap gap-1.5 px-3 py-1">
        {EFFECT_PRESETS.map((preset) => {
          const pressed = sameEffects(preset.effects, adjustmentsOf(picture));
          return (
            <button
              key={preset.id}
              type="button"
              aria-pressed={pressed}
              onClick={() =>
                effects.setPicture({ ...preset.effects, smoothMotion: picture.smoothMotion })
              }
              className={pill(pressed)}
            >
              {t(`videoPlayer.${preset.id}`)}
            </button>
          );
        })}
      </div>
      <PanelHeading>{t('videoPlayer.effectsLook')}</PanelHeading>
      <div className="flex flex-wrap gap-1.5 px-3 py-1">
        {LOOKS.map((look) => (
          <button
            key={look}
            type="button"
            aria-pressed={picture.look === look}
            onClick={() => effects.setPicture({ ...picture, look })}
            className={pill(picture.look === look)}
          >
            {t(LOOK_LABEL[look])}
          </button>
        ))}
      </div>
      <PanelHeading>{t('videoPlayer.effectsAdjust')}</PanelHeading>
      {ADJUSTMENTS.map((adjustment) => (
        <EffectSlider
          key={adjustment.key}
          name={t(ADJUSTMENT_LABEL[adjustment.key])}
          adjustment={adjustment}
          value={picture[adjustment.key]}
          onChange={(value) => effects.setPicture({ ...picture, [adjustment.key]: value })}
        />
      ))}
      <PanelHeading>{t('videoPlayer.effectsMotion')}</PanelHeading>
      <PanelOption
        toggle
        label={t('videoPlayer.effectSmoothMotion')}
        hint={t('videoPlayer.effectSmoothMotionHint')}
        selected={picture.smoothMotion}
        onSelect={() => effects.setPicture({ ...picture, smoothMotion: !picture.smoothMotion })}
      />
      <div className="flex flex-wrap gap-2 px-3 pt-3">
        <button type="button" onClick={effects.resetPicture} className={pill(false)}>
          {t('videoPlayer.effectsResetAll')}
        </button>
        <button type="button" onClick={effects.makePictureDefault} className={pill(false)}>
          {t('videoPlayer.effectsMakeDefault')}
        </button>
        {effects.ownPicture && (
          <button type="button" onClick={effects.useDefaultPicture} className={pill(false)}>
            {t('videoPlayer.effectsUseDefault')}
          </button>
        )}
      </div>
      {note && <p className="px-3 pt-3 text-xs text-white/60">{note}</p>}
      <SmartAudioOptions effects={effects} title={t('videoPlayer.effectsSound')} />
    </PlayerPanel>
  );
};

type SmartAudioProps = { effects: VideoEffects; title?: string };

/** Smart audio's switches: under the Audio panel's tracks, and as the Effects panel's Sound. */
export const SmartAudioOptions: FC<SmartAudioProps> = ({ effects, title }) => {
  const { t } = useTranslation();
  const { audio } = effects;
  return (
    <>
      <PanelHeading>{title ?? t('videoPlayer.smartAudio')}</PanelHeading>
      {SMART_AUDIO.map(({ key, label, hint }) => (
        <PanelOption
          key={key}
          toggle
          label={t(label)}
          hint={t(hint)}
          selected={audio[key]}
          onSelect={() => effects.setAudio({ ...audio, [key]: !audio[key] })}
        />
      ))}
      {effects.ownAudio && (
        <PanelOption
          label={t('videoPlayer.effectsMakeDefault')}
          onSelect={effects.makeAudioDefault}
        />
      )}
    </>
  );
};
