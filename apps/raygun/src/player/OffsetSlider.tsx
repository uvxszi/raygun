import { RotateCcw } from 'lucide-react';
import { useId, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

type OffsetSliderProps = {
  title: string;
  /** How far from the size by screen, in `unit`s: 0 is the size by screen. */
  value: number;
  min: number;
  max: number;
  step: number;
  /** `%` or `px`, shown after the number. */
  unit: string;
  onChange: (value: number) => void;
  disabled?: boolean;
};

/**
 * A size that starts from the screen's and moves from there: a slider and a number to type,
 * with a way back to the screen's own size. The same row as Settings' `PrefSlider`, with the
 * number field beside it; saved when let go, so dragging doesn't write every step.
 */
export const OffsetSlider: FC<OffsetSliderProps> = ({
  title,
  value,
  min,
  max,
  step,
  unit,
  onChange,
  disabled,
}) => {
  const { t } = useTranslation();
  const titleId = useId();
  const [draft, setDraft] = useState<number | null>(null);
  const [typed, setTyped] = useState<string | null>(null);
  const shown = draft ?? value;
  const clamp = (next: number) => Math.min(max, Math.max(min, Math.round(next / step) * step));
  const commit = (next: number | null) => {
    setDraft(null);
    setTyped(null);
    if (next !== null && Number.isFinite(next) && clamp(next) !== value) onChange(clamp(next));
  };
  const label = (offset: number) =>
    offset === 0
      ? t('player.view.byScreen')
      : t('player.view.offset', { sign: offset > 0 ? '+' : '−', n: Math.abs(offset), unit });

  return (
    <li className="flex flex-col gap-2 px-1 py-3">
      <div className="flex items-center gap-3">
        <span id={titleId} className="min-w-0 flex-1 font-medium">
          {title}
          <span className="text-muted block text-sm font-normal">{label(shown)}</span>
        </span>
        <label className="bg-background flex shrink-0 items-center gap-1 rounded-lg px-2 py-1 text-sm">
          <input
            type="number"
            inputMode="numeric"
            aria-label={t('player.view.offsetField', { name: title, unit })}
            min={min}
            max={max}
            step={step}
            value={typed ?? String(shown)}
            disabled={disabled}
            onChange={(event) => setTyped(event.target.value)}
            onBlur={() => commit(typed === null ? null : Number(typed))}
            onKeyDown={(event) => {
              if (event.key === 'Enter') commit(typed === null ? null : Number(typed));
            }}
            className="w-12 bg-transparent text-right tabular-nums outline-none"
          />
          <span className="text-muted">{unit}</span>
        </label>
        <button
          type="button"
          aria-label={t('player.view.reset', { name: title })}
          title={t('player.view.byScreen')}
          disabled={disabled || shown === 0}
          onClick={() => commit(0)}
          className="text-muted hover:text-foreground grid size-8 shrink-0 place-items-center rounded-full disabled:opacity-30"
        >
          <RotateCcw aria-hidden size={15} />
        </button>
      </div>
      <input
        type="range"
        aria-labelledby={titleId}
        aria-valuetext={label(shown)}
        min={min}
        max={max}
        step={step}
        value={shown}
        disabled={disabled}
        onChange={(event) => setDraft(Number(event.target.value))}
        onPointerUp={() => commit(draft)}
        onKeyUp={() => commit(draft)}
        onBlur={() => commit(draft)}
        className="accent-primary w-full"
      />
    </li>
  );
};
