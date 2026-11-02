import { RotateCcw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { EQ_FREQUENCIES } from '../engine/soundGraph';
import { formatDb } from './format';
import { EQ_PRESETS, presetFor } from './presets';
import type { FullSound } from './soundPrefs';

/** ±12 dB in tenths. */
const LIMIT = 120;

const frequencyLabel = (hertz: number) => (hertz >= 1000 ? `${hertz / 1000}k` : `${hertz}`);
type EqualizerPanelProps = {
  sound: FullSound;
  change: (patch: Partial<FullSound>) => void;
};

/**
 * The equalizer (M46.12): presets, ten bands as vertical sliders over their curve, and the
 * preamp. Changing a band makes the preset "custom"; a preset turns the equalizer on, as in RiPlay.
 */
export const EqualizerPanel: FC<EqualizerPanelProps> = ({ sound, change }) => {
  const { t } = useTranslation();
  const setBand = (index: number, gain: number) => {
    const eqBands = sound.eqBands.map((current, at) => (at === index ? gain : current));
    change({ eqBands, eqPreset: presetFor(eqBands), eqEnabled: true });
  };
  const points = sound.eqBands
    .map((gain, index) => {
      const x = ((index + 0.5) / EQ_FREQUENCIES.length) * 100;
      const y = 50 - (gain / LIMIT) * 45;
      return `${x},${y}`;
    })
    .join(' ');

  return (
    <div data-testid="equalizer" className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-3">
        <label className="flex items-center gap-3 text-sm font-medium">
          <input
            type="checkbox"
            role="switch"
            checked={sound.eqEnabled}
            onChange={(event) => change({ eqEnabled: event.target.checked })}
            className="accent-primary size-4"
          />
          {t('sound.eqOn')}
        </label>
        <button
          type="button"
          onClick={() =>
            change({ eqBands: EQ_PRESETS[0]?.bands ?? [], eqPreset: 'flat', preamp: 0 })
          }
          className="text-muted hover:text-foreground flex items-center gap-1.5 text-sm"
        >
          <RotateCcw aria-hidden size={14} />
          {t('sound.reset')}
        </button>
      </div>

      <div role="group" aria-label={t('sound.presets')} className="flex flex-wrap gap-2">
        {EQ_PRESETS.map((preset) => {
          const chosen = sound.eqEnabled && sound.eqPreset === preset.id;
          return (
            <button
              key={preset.id}
              type="button"
              aria-pressed={chosen}
              onClick={() =>
                change({ eqBands: preset.bands, eqPreset: preset.id, eqEnabled: true })
              }
              className={`rounded-full px-3 py-1.5 text-sm ${
                chosen
                  ? 'bg-primary text-background font-semibold'
                  : 'bg-background hover:bg-surface ring-muted/30 ring-1'
              }`}
            >
              {t(`sound.preset.${preset.id}`)}
            </button>
          );
        })}
        {sound.eqPreset === 'custom' && (
          <span className="bg-primary/15 text-primary rounded-full px-3 py-1.5 text-sm">
            {t('sound.preset.custom')}
          </span>
        )}
      </div>

      <div
        className={`relative grid grid-cols-10 gap-1 rounded-2xl px-2 pt-2 pb-1 ${
          sound.eqEnabled ? '' : 'opacity-50'
        }`}
      >
        <svg
          aria-hidden
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          className="pointer-events-none absolute inset-x-2 top-8 h-40 w-[calc(100%-1rem)]"
        >
          <line x1="0" y1="50" x2="100" y2="50" className="stroke-muted/30" strokeWidth="0.5" />
          <polyline
            points={points}
            fill="none"
            className="stroke-primary"
            strokeWidth="1.5"
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        {EQ_FREQUENCIES.map((hertz, index) => {
          const gain = sound.eqBands[index] ?? 0;
          return (
            <div key={hertz} className="flex flex-col items-center gap-1">
              <span className="text-muted text-[10px] tabular-nums">
                {gain > 0 ? '+' : ''}
                {(gain / 10).toFixed(gain % 10 === 0 ? 0 : 1)}
              </span>
              <input
                type="range"
                aria-label={t('sound.band', { frequency: `${frequencyLabel(hertz)}Hz` })}
                aria-valuetext={formatDb(gain)}
                min={-LIMIT}
                max={LIMIT}
                step={5}
                value={gain}
                onChange={(event) => setBand(index, Number(event.target.value))}
                onDoubleClick={() => setBand(index, 0)}
                className="accent-primary h-40 w-6 cursor-pointer [direction:rtl] [writing-mode:vertical-lr]"
              />
              <span className="text-muted text-[11px] tabular-nums">{frequencyLabel(hertz)}</span>
            </div>
          );
        })}
      </div>

      <label className="flex flex-col gap-1 text-sm">
        <span className="flex justify-between">
          <span>{t('sound.preamp')}</span>
          <span className="text-primary font-semibold tabular-nums">{formatDb(sound.preamp)}</span>
        </span>
        <input
          type="range"
          aria-label={t('sound.preamp')}
          aria-valuetext={formatDb(sound.preamp)}
          min={-LIMIT}
          max={LIMIT}
          step={5}
          value={sound.preamp}
          onChange={(event) => change({ preamp: Number(event.target.value), eqEnabled: true })}
          className="accent-primary w-full"
        />
        <span className="text-muted text-xs">{t('sound.preampHint')}</span>
      </label>
    </div>
  );
};
