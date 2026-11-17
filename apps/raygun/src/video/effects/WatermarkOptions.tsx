import { RotateCcw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { PanelHeading, PanelOption } from '../PlayerPanel';
import type { VideoEffects } from './useVideoEffects';
import type { Watermark } from './watermarkMath';

const pill = 'rounded-full bg-white/10 px-3 py-1.5 text-sm hover:bg-white/20';

type Control = { key: 'brightness' | 'contrast' | 'feather'; label: string; min: number };

const CONTROLS: Control[] = [
  { key: 'brightness', label: 'videoPlayer.watermarkBrightness', min: -100 },
  { key: 'contrast', label: 'videoPlayer.watermarkContrast', min: -100 },
  { key: 'feather', label: 'videoPlayer.watermarkFeather', min: 0 },
];

const signed = (value: number, min: number) =>
  min === 0 || value === 0 ? String(value) : value > 0 ? `+${value}` : `−${-value}`;

type Props = {
  effects: VideoEffects;
  /** Draw a box: the panel makes room for the picture. */
  onDraw: () => void;
};

/**
 * The Effects panel's watermark remover (the title's own): on or off, a box drawn over the
 * picture or found by Raygun (Find it), and brightness, contrast and feather for what's left.
 */
export const WatermarkOptions: FC<Props> = ({ effects, onDraw }) => {
  const { t } = useTranslation();
  const { watermark } = effects.picture;
  const hasBox = watermark.width > 0 && watermark.height > 0;
  const set = (next: Watermark) => effects.setWatermark(next);
  const status = effects.watermarkStatus;
  const line =
    status === 'finding'
      ? t('videoPlayer.watermarkFinding')
      : status === 'notFound'
        ? t('videoPlayer.watermarkNotFound')
        : hasBox && status === 'learning'
          ? t('videoPlayer.watermarkLearning')
          : null;
  return (
    <>
      <PanelHeading>{t('videoPlayer.effectsWatermark')}</PanelHeading>
      <PanelOption
        toggle
        label={t('videoPlayer.watermarkRemove')}
        hint={t('videoPlayer.watermarkHint')}
        selected={watermark.enabled}
        onSelect={() => set({ ...watermark, enabled: !watermark.enabled })}
      />
      {watermark.enabled && (
        <>
          <div className="flex flex-wrap gap-2 px-3 py-1">
            <button
              type="button"
              className={pill}
              onClick={() => {
                effects.setDrawingWatermark(true);
                onDraw();
              }}
            >
              {hasBox ? t('videoPlayer.watermarkRedraw') : t('videoPlayer.watermarkDraw')}
            </button>
            <button
              type="button"
              className={pill}
              disabled={status === 'finding'}
              onClick={() => effects.setWatermarkStatus('finding')}
            >
              {t('videoPlayer.watermarkFind')}
            </button>
            {hasBox && (
              <button
                type="button"
                className={pill}
                onClick={() => set({ ...watermark, x: 0, y: 0, width: 0, height: 0 })}
              >
                {t('videoPlayer.watermarkClear')}
              </button>
            )}
          </div>
          {line && (
            <p role="status" className="px-3 py-1 text-xs text-white/60">
              {line}
            </p>
          )}
          {hasBox &&
            CONTROLS.map(({ key, label, min }) => {
              const name = t(label);
              const value = watermark[key];
              const change = (next: number) =>
                effects.setPicture({
                  ...effects.picture,
                  watermark: { ...watermark, [key]: next },
                });
              return (
                <div key={key} className="flex items-end gap-1 px-3 py-1">
                  <div className="flex min-w-0 flex-1 flex-col gap-1 text-sm">
                    <div className="flex justify-between gap-2">
                      <span className="truncate">{name}</span>
                      <span className="text-white/70 tabular-nums">{signed(value, min)}</span>
                    </div>
                    <input
                      type="range"
                      aria-label={name}
                      min={min}
                      max={100}
                      step={1}
                      value={value}
                      onChange={(event) => change(Number(event.target.value))}
                      className="accent-primary w-full"
                    />
                  </div>
                  <button
                    type="button"
                    aria-label={t('settings.sizeReset', { name })}
                    disabled={value === 0}
                    onClick={() => change(0)}
                    className="grid size-8 place-items-center rounded-full hover:bg-white/10 disabled:opacity-30"
                  >
                    <RotateCcw aria-hidden size={14} />
                  </button>
                </div>
              );
            })}
        </>
      )}
    </>
  );
};
